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
    ptr::SendMut,
    sync::OneShot,
};

struct RecvMsgCompletion {
    fd: RawFd,
    addr: Pin<Box<SocketAddrC>>,
    iovecs: Pin<Vec<IoVec>>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<(usize, SocketAddr)>>,
}

impl Completion for RecvMsgCompletion {
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

/// This represents a single use asynchronous receive message operation. This will return the total
/// numbe of bytes read in across all supplied vectors, and the socket address the data was
/// received from. Users should read data from the first supplied buffer and continue until all
/// read data has been handled.
pub struct RecvMsg<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<(usize, SocketAddr)>>,
}

impl<'a, T> Drop for RecvMsg<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> RecvMsg<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new<'b>(sock: &'a mut T, bufs: &'b mut [Vec<u8>]) -> RecvMsg<'a, T> {
        let result = OneShot::new();

        let (addr, addr_len) = SocketAddrC::new();
        let mut addr = Box::pin(addr);

        let mut iovecs = Vec::with_capacity(bufs.len());
        for buf in bufs.iter_mut() {
            iovecs.push(IoVec {
                iov_base: unsafe { SendMut::new(buf.as_mut_ptr() as _) },
                iov_len: buf.len(),
            })
        }
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

        let op = RecvMsgCompletion {
            fd: sock.as_raw_fd(),
            addr,
            iovecs,
            hdr,
            result: result.clone(),
        };
        let id = context::uring().register(op);

        RecvMsg {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for RecvMsg<'a, T>
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
