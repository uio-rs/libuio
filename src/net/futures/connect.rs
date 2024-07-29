use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use ::io_uring::{cqueue, opcode, squeue, types};
use futures::Future;
use nix::libc;

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    net::SocketAddrC,
    sync::OneShot,
};

struct ConnectCompletion {
    addr: Pin<Box<SocketAddrC>>,
    addr_len: libc::socklen_t,
    fd: RawFd,
    result: OneShot<io::Result<()>>,
}

impl Completion for ConnectCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok(()),
        };

        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Connect::new(types::Fd(self.fd), self.addr.as_ptr(), self.addr_len).build()
    }
}

/// This represents a single use asynchronous connect operation to create a new [TcpStream] object
/// to interact with a remote host on. This will ultimately return the connected and ready to use
/// [TcpStream].
pub struct Connect<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<()>>,
}

impl<'a, T> Drop for Connect<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> Connect<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(sock: &'a mut T, remote: &SocketAddr) -> Connect<'a, T> {
        let (addr, addr_len) = SocketAddrC::from_std(remote);
        let addr = Box::pin(addr);

        let result = OneShot::new();
        let op = ConnectCompletion {
            addr,
            addr_len,
            fd: sock.as_raw_fd(),
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

        Connect {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for Connect<'a, T>
where
    T: AsRawFd,
{
    type Output = io::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);

        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
