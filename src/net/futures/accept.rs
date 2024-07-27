use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use io_uring::{cqueue, opcode, squeue, types};
use nix::libc;

use crate::{
    context,
    io_uring::{Completion, CompletionStatus},
    net::{SocketAddrC, TcpStream},
    sync::OneShot,
};

struct AcceptCompletion {
    fd: RawFd,
    addr: Pin<Box<SocketAddrC>>,
    addr_len: libc::socklen_t,
    conn: OneShot<io::Result<(OwnedFd, SocketAddr)>>,
}

impl AcceptCompletion {
    pub fn new(fd: RawFd, result: OneShot<io::Result<(OwnedFd, SocketAddr)>>) -> AcceptCompletion {
        let (addr, addr_len) = SocketAddrC::new();
        let addr = Box::pin(addr);
        AcceptCompletion {
            fd,
            addr,
            addr_len,
            conn: result,
        }
    }
}

impl Completion for AcceptCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => {
                let addr = self.addr.as_std();
                Ok((unsafe { OwnedFd::from_raw_fd(result) }, addr))
            }
        };

        self.conn.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Accept::new(
            types::Fd(self.fd),
            self.addr.as_mut_ptr(),
            (&mut self.addr_len) as *mut u32,
        )
        .build()
    }
}

/// This represents a single use future for accepting an active conntion from a live [TcpListener].
/// When polled to completion the future will return a valid [TcpStream], or any [std::io::Error]
/// encountered while awaiting the new connection.
pub struct Accept<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<(OwnedFd, SocketAddr)>>,
}

impl<'a, T> Drop for Accept<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> Accept<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(listener: &'a mut T) -> Accept<'a, T> {
        let result = OneShot::new();
        let op = AcceptCompletion::new(listener.as_raw_fd(), result.clone());
        let id = context::uring().register(op);

        Accept {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for Accept<'a, T>
where
    T: AsRawFd,
{
    type Output = io::Result<TcpStream>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result.map(TcpStream::from)),
            None => Poll::Pending,
        }
    }
}
