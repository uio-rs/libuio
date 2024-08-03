use ::std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use ::futures::Future;
use ::io_uring::{cqueue, opcode, squeue, types};

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    sync::OneShot,
};

struct RecvCompletion {
    fd: RawFd,
    buf: Vec<u8>,
    buf_len: u32,
    result: OneShot<io::Result<Vec<u8>>>,
}

impl Completion for RecvCompletion {
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus {
        // This is safe and _very_ efficient, since the take call uses the
        // Vec::default implementation which does 0 allocations.
        let mut buf = std::mem::take(&mut self.buf);

        // Pull out the result and check the response code, if we are negative it
        // represents an error so hand off to the io::Error setup. Otherwise we got
        // something back.
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => {
                let len = result as usize;

                // SAFETY: Since we own the Vec<u8> here and the OS has informed us that
                // its done with the pointer, and guarantees that 0..len bytes are
                // initialized, we can safely call [Vec::set_len] because both of its
                // invariants hold true:
                // - The elements at `old_len..new_len` are initialized by the OS.
                // - And our length is less than or equal to our capacity, as the OS won't
                // write past the capacity we define.
                debug_assert!(len <= buf.capacity(), "The OS LIES!!!");
                unsafe { buf.set_len(len) };
                Ok(buf)
            }
        };

        // Pass off the result back to the originating Future, and then return Finalized as
        // we are a OneShot based completion.
        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::Recv::new(types::Fd(self.fd), self.buf.as_mut_ptr(), self.buf_len).build()
    }
}

/// This represents a single use asynchronous receive on a connected [TcpStream], it will use the
/// given buffer to read data into, and ultimately return the amount of data read and whether or
/// not ther was still data in the socket after the receive completed.
pub struct Recv<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<Vec<u8>>>,
}

impl<'a, T> Drop for Recv<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> Recv<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(stream: &'a mut T, buf: Vec<u8>) -> Recv<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.capacity() as u32;

        let op = RecvCompletion {
            fd: stream.as_raw_fd(),
            buf,
            buf_len,
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

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
    type Output = io::Result<Vec<u8>>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
