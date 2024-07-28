use std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    ptr,
    task::{Context, Poll},
};

use futures::Future;
use io_uring::{cqueue, opcode, squeue, types};

use crate::{
    context,
    io_uring::{Completion, CompletionStatus},
    net::{IoVec, MsgHdr, SocketAddrC},
    sync::OneShot,
    util::SendMut,
};

struct RecvFromCompletion {
    fd: RawFd,
    addr: Pin<Box<SocketAddrC>>,
    iovecs: Pin<Vec<IoVec>>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<(usize, SocketAddr)>>,
}

impl Completion for RecvFromCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok((result as usize, self.addr.as_std())),
        };

        assert!(self.iovecs.len() > 0);
        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::RecvMsg::new(types::Fd(self.fd), self.hdr.as_mut_ptr()).build()
    }
}

/// This represents a single use asynchronous receive from operation, this will return both the
/// number of bytes read as well as the socket address that the data was received from.
pub struct RecvFrom<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<(usize, SocketAddr)>>,
}

impl<'a, T> Drop for RecvFrom<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> RecvFrom<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new<'b>(sock: &'a mut T, buf: &'b mut [u8]) -> RecvFrom<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.len();
        let buf = unsafe { SendMut::new(buf.as_mut_ptr() as _) };

        let (addr, addr_len) = SocketAddrC::new();
        let mut addr = Box::pin(addr);

        let iovecs = vec![IoVec {
            iov_base: buf,
            iov_len: buf_len,
        }];
        let mut iovecs = Pin::new(iovecs);

        let hdr = MsgHdr {
            msg_name: unsafe { SendMut::new(addr.as_mut_ptr() as _) },
            msg_namelen: addr_len,
            msg_iov: unsafe { SendMut::new(iovecs.as_mut_ptr()) },
            msg_iovlen: iovecs.len(),
            msg_control: unsafe { SendMut::new(ptr::null_mut()) },
            msg_controllen: 0,
            msg_flags: 0,
        };
        let hdr = Box::pin(hdr);

        let op = RecvFromCompletion {
            fd: sock.as_raw_fd(),
            addr,
            iovecs,
            hdr,
            result: result.clone(),
        };
        let id = context::uring().register(op);

        RecvFrom {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for RecvFrom<'a, T>
where
    T: AsRawFd,
{
    type Output = io::Result<(usize, SocketAddr)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
