use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use io_uring::{cqueue, opcode, squeue, types};

use crate::{
    context,
    io_uring::{Completion, CompletionStatus},
    sync::OneShot,
    util::SendConst,
};

struct SendCompletion {
    fd: RawFd,
    buf: SendConst<u8>,
    buf_len: u32,
    result: OneShot<io::Result<usize>>,
}

impl Completion for SendCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok(result as usize),
        };

        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Send::new(types::Fd(self.fd), self.buf.to_ptr(), self.buf_len).build()
    }
}

/// This represents a single use asynchronous send operation on a connected [TcpStream], it will
/// use the given buffer to write data from, and ultimately return the amount of data written to
/// the remote server.
pub struct Send<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<usize>>,
}

impl<'a, T> Drop for Send<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> Send<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new<'buf>(stream: &'a mut T, buf: &'buf [u8]) -> Send<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.len() as u32;
        let buf = unsafe { SendConst::new(buf.as_ptr()) };
        let op = SendCompletion {
            fd: stream.as_raw_fd(),
            buf,
            buf_len,
            result: result.clone(),
        };
        let id = context::uring().register(op);

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
    type Output = io::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
