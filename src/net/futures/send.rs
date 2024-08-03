use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use ::io_uring::{cqueue, opcode, squeue, types};
use futures::Future;

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    sync::OneShot,
};

struct SendCompletion {
    fd: RawFd,
    buf: Vec<u8>,
    buf_len: u32,
    result: OneShot<io::Result<(usize, Vec<u8>)>>,
}

impl Completion for SendCompletion {
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus {
        let buf = std::mem::take(&mut self.buf);

        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok((result as usize, buf)),
        };

        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Send::new(types::Fd(self.fd), self.buf.as_ptr(), self.buf_len).build()
    }
}

/// This represents a single use asynchronous send operation on a connected [TcpStream], it will
/// use the given buffer to write data from, and ultimately return the amount of data written to
/// the remote server.
pub struct Send<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<(usize, Vec<u8>)>>,
}

impl<'a, T> Drop for Send<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> Send<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(stream: &'a mut T, buf: Vec<u8>) -> Send<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.len() as u32;
        let op = SendCompletion {
            fd: stream.as_raw_fd(),
            buf,
            buf_len,
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

        Send {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for Send<'a, T>
where
    T: AsRawFd,
{
    type Output = io::Result<(usize, Vec<u8>)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
