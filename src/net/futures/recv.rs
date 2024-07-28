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
    ptr::SendMut,
    sync::OneShot,
};

struct RecvCompletion {
    fd: RawFd,
    buf: SendMut<u8>,
    buf_len: u32,
    result: OneShot<io::Result<usize>>,
}

impl Completion for RecvCompletion {
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
        opcode::Recv::new(types::Fd(self.fd), self.buf.to_ptr(), self.buf_len).build()
    }
}

/// This represents a single use asynchronous receive on a connected [TcpStream], it will use the
/// given buffer to read data into, and ultimately return the amount of data read and whether or
/// not ther was still data in the socket after the receive completed.
pub struct Recv<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<usize>>,
}

impl<'a, T> Drop for Recv<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> Recv<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new<'buf>(stream: &'a mut T, buf: &'buf mut [u8]) -> Recv<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.len() as u32;
        let buf = unsafe { SendMut::new(buf.as_mut_ptr()) };

        let op = RecvCompletion {
            fd: stream.as_raw_fd(),
            buf,
            buf_len,
            result: result.clone(),
        };
        let id = context::uring().register(op);

        Recv {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for Recv<'a, T>
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
