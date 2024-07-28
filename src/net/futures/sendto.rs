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

struct SendToCompletion {
    fd: RawFd,
    addr: Option<Pin<Box<SocketAddrC>>>,
    iovecs: Pin<Vec<IoVec>>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<usize>>,
}

impl Completion for SendToCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok(result as usize),
        };

        assert!(self.iovecs.len() > 0);
        if let Some(addr) = &self.addr {
            assert!(addr.is_valid());
        }
        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> squeue::Entry {
        opcode::SendMsg::new(types::Fd(self.fd), self.hdr.as_mut_ptr()).build()
    }
}

/// This represents a single use send to operation. This will return the number of bytes sent
/// across the supplied buffers. Specifying the send to address is optional on connected sockets.
pub struct SendTo<'a, T> {
    inner: PhantomData<&'a mut T>,
    id: usize,
    result: OneShot<io::Result<usize>>,
}

impl<'a, T> Drop for SendTo<'a, T> {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl<'a, T> SendTo<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new<'b>(
        sock: &'a mut T,
        buf: &'b mut [u8],
        addr: Option<&SocketAddr>,
    ) -> SendTo<'a, T> {
        let result = OneShot::new();
        let buf_len = buf.len();
        let buf = unsafe { SendMut::new(buf.as_mut_ptr() as _) };

        let (addr, addr_ptr, addr_len) = match addr {
            Some(addr) => {
                let (addr, addr_len) = SocketAddrC::from_std(addr);
                let mut addr = Box::pin(addr);
                let addr_ptr = addr.as_mut_ptr();
                (Some(addr), addr_ptr as _, addr_len)
            }
            None => (None, ptr::null_mut(), 0),
        };

        let iovecs = vec![IoVec {
            iov_base: buf,
            iov_len: buf_len,
        }];
        let mut iovecs = Pin::new(iovecs);

        let hdr = MsgHdr {
            msg_name: unsafe { SendMut::new(addr_ptr as _) },
            msg_namelen: addr_len,
            msg_iov: unsafe { SendMut::new(iovecs.as_mut_ptr()) },
            msg_iovlen: iovecs.len(),
            msg_control: unsafe { SendMut::new(ptr::null_mut()) },
            msg_controllen: 0,
            msg_flags: 0,
        };
        let hdr = Box::pin(hdr);

        let op = SendToCompletion {
            fd: sock.as_raw_fd(),
            addr,
            iovecs,
            hdr,
            result: result.clone(),
        };
        let id = context::uring().register(op);

        SendTo {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl<'a, T> Future for SendTo<'a, T>
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
