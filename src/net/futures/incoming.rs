use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    pin::Pin,
    sync::mpsc::TryRecvError,
    task::{Context, Poll},
};

use ::io_uring::{cqueue, opcode, squeue, types};
use futures::Stream;

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    net::TcpStream,
    sync::{channel, Receiver, Sender},
};

struct IncomingCompletion {
    fd: RawFd,
    result: Sender<io::Result<OwnedFd>>,
}

impl Completion for IncomingCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok(unsafe { OwnedFd::from_raw_fd(result) }),
        };

        match self.result.push(result) {
            Err(_) => CompletionStatus::Finalized,
            Ok(_) if cqueue::more(value.flags()) => CompletionStatus::Armed,
            Ok(_) => CompletionStatus::Rearm,
        }
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::AcceptMulti::new(types::Fd(self.fd)).build()
    }
}

/// This represents a stream future of incoming [TcpStream] connections. This will continue to
/// return connections until either the future is dropped, or there is an unrecoverable error
/// enountered.
///
/// Note this future is meant to be reused, so ensure that when in use that its lifetime extends
/// beyond any loops in use.
pub struct Incoming<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    stream: Receiver<io::Result<OwnedFd>>,
}

impl<'a, T> Drop for Incoming<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> Incoming<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(listener: &'a mut T) -> Incoming<'a, T> {
        let (tx, rx) = channel();
        let op = IncomingCompletion {
            fd: listener.as_raw_fd(),
            result: tx,
        };
        let id = io_uring::uring().register(op);

        Incoming {
            inner: PhantomData,
            id,
            stream: rx,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.stream.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Stream for Incoming<'a, T>
where
    T: AsRawFd,
{
    type Item = io::Result<TcpStream>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.set_waker(cx);
        match self.stream.try_recv() {
            Ok(val) => Poll::Ready(Some(val.map(TcpStream::from))),
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => Poll::Ready(None),
        }
    }
}
