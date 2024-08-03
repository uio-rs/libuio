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

struct RecvMsgCompletion {
    fd: RawFd,
    addr: Pin<Box<SocketAddrC>>,
    bufs: Vec<Vec<u8>>,
    iovecs: Vec<IoVec>,
    hdr: Pin<Box<MsgHdr>>,
    result: OneShot<io::Result<(Vec<Vec<u8>>, SocketAddr)>>,
}

impl Completion for RecvMsgCompletion {
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus {
        // This is safe and _very_ efficient, since the take call uses the
        // Vec::default implementation which does 0 allocations.
        let mut bufs = std::mem::take(&mut self.bufs);

        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => {
                let mut len = result as usize;

                // SAFETY: Since we own the Vec<u8> here and the OS has informed us that
                // its done with the pointer, and guarantees that 0..len bytes are
                // initialized, we can safely call [Vec::set_len] because both of its
                // invariants hold true:
                // - The elements at `old_len..new_len` are initialized by the OS.
                // - And our length is less than or equal to our capacity, as the OS won't
                // write past the capacity we define.
                for buf in bufs.iter_mut() {
                    let buf_len = len.min(buf.capacity());
                    len -= buf_len;

                    if buf_len > 0 {
                        unsafe { buf.set_len(buf_len) };
                    } else {
                        unsafe { buf.set_len(0) };
                    }
                }
                Ok((bufs, self.addr.as_std()))
            }
        };

        assert!(!self.iovecs.is_empty());
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
    result: OneShot<io::Result<(Vec<Vec<u8>>, SocketAddr)>>,
}

impl<'a, T> Drop for RecvMsg<'a, T> {
    fn drop(&mut self) {
        io_uring::uring().deregister(self.id);
    }
}

impl<'a, T> RecvMsg<'a, T>
where
    T: AsRawFd,
{
    pub(crate) fn new(sock: &'a mut T, mut bufs: Vec<Vec<u8>>) -> RecvMsg<'a, T> {
        let result = OneShot::new();

        let (addr, addr_len) = SocketAddrC::new();
        let mut addr = Box::pin(addr);

        let mut iovecs = Vec::with_capacity(bufs.len());
        for buf in bufs.iter_mut() {
            iovecs.push(IoVec {
                iov_base: buf.as_mut_ptr() as _,
                iov_len: buf.len(),
            });
        }

        let hdr = MsgHdr {
            msg_name: addr.as_mut_ptr() as _,
            msg_namelen: addr_len,
            msg_iov: iovecs.as_mut_ptr() as _,
            msg_iovlen: iovecs.len(),
            msg_control: ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };
        let hdr = Box::pin(hdr);

        let op = RecvMsgCompletion {
            fd: sock.as_raw_fd(),
            addr,
            bufs,
            iovecs,
            hdr,
            result: result.clone(),
        };
        let id = io_uring::uring().register(op);

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
    type Output = io::Result<(Vec<Vec<u8>>, SocketAddr)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
