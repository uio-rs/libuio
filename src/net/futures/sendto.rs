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

use ::io_uring::{cqueue, opcode, squeue, types};
use futures::Future;

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    net::{IoVec, MsgHdr, SocketAddrC},
    sync::OneShot,
};

struct SendToCompletion {
    fd: RawFd,
    addr: Option<Pin<Box<SocketAddrC>>>,
    buf: Vec<u8>,
    iovec: Pin<Box<IoVec>>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<(usize, Vec<u8>)>>,
}

impl Completion for SendToCompletion {
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus {
        let buf = std::mem::take(&mut self.buf);

        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => Ok((result as usize, buf)),
        };

        assert!(self.iovec.iov_len > 0);
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
    result: OneShot<io::Result<(usize, Vec<u8>)>>,
}

impl<'a, T> Drop for SendTo<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> SendTo<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(
        sock: &'a mut T,
        mut buf: Vec<u8>,
        addr: Option<SocketAddr>,
    ) -> SendTo<'a, T> {
        let result = OneShot::new();

        let (addr, addr_ptr, addr_len) = match addr {
            Some(addr) => {
                let (addr, addr_len) = SocketAddrC::from_std(&addr);
                let mut addr = Box::pin(addr);
                let addr_ptr = addr.as_mut_ptr();
                (Some(addr), addr_ptr as _, addr_len)
            }
            None => (None, ptr::null_mut(), 0),
        };

        let iovec = IoVec {
            iov_base: buf.as_mut_ptr() as _,
            iov_len: buf.len(),
        };
        let mut iovec = Box::pin(iovec);

        let hdr = MsgHdr {
            msg_name: addr_ptr,
            msg_namelen: addr_len,
            msg_iov: iovec.as_mut_ptr(),
            msg_iovlen: 1,
            msg_control: ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };
        let hdr = Box::pin(hdr);

        let op = SendToCompletion {
            fd: sock.as_raw_fd(),
            addr,
            buf,
            iovec,
            hdr,
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

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
    type Output = io::Result<(usize, Vec<u8>)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
