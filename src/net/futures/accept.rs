use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    pin::Pin,
    ptr,
    task::{Context, Poll},
};

use ::io_uring::{cqueue, opcode, squeue, types};
use futures::Future;

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    net::TcpStream,
    sync::OneShot,
};

struct AcceptCompletion {
    fd: RawFd,
    conn: OneShot<io::Result<OwnedFd>>,
}

impl AcceptCompletion {
    pub fn new(fd: RawFd, result: OneShot<io::Result<OwnedFd>>) -> AcceptCompletion {
        AcceptCompletion { fd, conn: result }
    }
}

impl Completion for AcceptCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok(unsafe { OwnedFd::from_raw_fd(result) }),
        };

        self.conn.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Accept::new(types::Fd(self.fd), ptr::null_mut(), ptr::null_mut()).build()
    }
}

/// This represents a single use future for accepting an active conntion from a live [TcpListener].
/// When polled to completion the future will return a valid [TcpStream], or any [std::io::Error]
/// encountered while awaiting the new connection.
pub struct Accept<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<OwnedFd>>,
}

impl<'a, T> Drop for Accept<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> Accept<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(listener: &'a mut T) -> Accept<'a, T> {
        let result = OneShot::new();
        let op = AcceptCompletion::new(listener.as_raw_fd(), result.clone());
        let id = io_uring::uring().register(op);

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
