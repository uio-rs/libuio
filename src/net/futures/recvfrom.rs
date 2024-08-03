use ::std::{
    cmp::Ordering,
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    ptr,
    task::{Context, Poll},
};

use ::futures::Future;
use ::io_uring::{cqueue, opcode, squeue, types};

use crate::{
    io_uring::{self, Completion, CompletionStatus},
    net::{IoVec, MsgHdr, SocketAddrC},
    sync::OneShot,
};

struct RecvFromCompletion {
    fd: RawFd,
    addr: Pin<Box<SocketAddrC>>,
    buf: Vec<u8>,
    iovec: Pin<Box<IoVec>>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<(Vec<u8>, SocketAddr)>>,
}

impl Completion for RecvFromCompletion {
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus {
        // This is safe and _very_ efficient, since the take call uses the
        // Vec::default implementation which does 0 allocations.
        let mut buf = std::mem::take(&mut self.buf);

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
                Ok((buf, self.addr.as_std()))
            }
        };

        assert!(self.iovec.iov_len == 1);
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
    result: OneShot<io::Result<(Vec<u8>, SocketAddr)>>,
}

impl<'a, T> Drop for RecvFrom<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> RecvFrom<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(sock: &'a mut T, mut buf: Vec<u8>) -> RecvFrom<'a, T> {
        let result = OneShot::new();

        let (addr, addr_len) = SocketAddrC::new();
        let mut addr = Box::pin(addr);

        let iovec = IoVec {
            iov_base: buf.as_mut_ptr() as _,
            iov_len: buf.len(),
        };
        let mut iovec = Box::pin(iovec);

        let hdr = MsgHdr {
            msg_name: addr.as_mut_ptr() as _,
            msg_namelen: addr_len,
            msg_iov: iovec.as_mut_ptr(),
            msg_iovlen: 1,
            msg_control: ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };
        let hdr = Box::pin(hdr);

        let op = RecvFromCompletion {
            fd: sock.as_raw_fd(),
            addr,
            buf,
            iovec,
            hdr,
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

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
    type Output = io::Result<(Vec<u8>, SocketAddr)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
