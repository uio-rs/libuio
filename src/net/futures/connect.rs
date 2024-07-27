use std::{
    cmp::Ordering,
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use io_uring::{cqueue, opcode, types};
use nix::libc;

use crate::{
    context,
    io_uring::{Completion, CompletionStatus},
    net::{getsockname, socket, SocketAddrC, TcpStream},
    sync::OneShot,
};

struct ConnectCompletion {
    addr: Pin<Box<SocketAddrC>>,
    addr_len: libc::socklen_t,
    fd: RawFd,
    result: OneShot<io::Result<SocketAddr>>,
}

impl Completion for ConnectCompletion {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus {
        let result = value.result();
        let result = match result.cmp(&0) {
            Ordering::Less => Err(io::Error::from_raw_os_error(-result)),
            Ordering::Equal | Ordering::Greater => getsockname(self.fd),
        };

        self.result.complete(result);
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> io_uring::squeue::Entry {
        opcode::Connect::new(types::Fd(self.fd), self.addr.as_ptr(), self.addr_len).build()
    }
}

/// This represents a single use asynchronous connect operation to create a new [TcpStream] object
/// to interact with a remote host on. This will ultimately return the connected and ready to use
/// [TcpStream].
pub struct Connect {
    fd: Option<OwnedFd>,
    id: usize,
    result: OneShot<io::Result<SocketAddr>>,
}

impl Drop for Connect {
    fn drop(&mut self) {
        context::uring().deregister(self.id);
    }
}

impl Connect {
    pub(crate) fn new(remote: SocketAddr) -> io::Result<Connect> {
        let fd = socket::client_socket(remote)?;
        let (addr, addr_len) = SocketAddrC::from_std(&remote);
        let addr = Box::pin(addr);

        let result = OneShot::new();
        let op = ConnectCompletion {
            addr,
            addr_len,
            fd: fd.as_raw_fd(),
            result: result.clone(),
        };
        let id = context::uring().register(op);

        Ok(Connect {
            fd: Some(fd),
            id,
            result,
        })
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        self.result.set_waker(cx.waker().clone());
    }
}

impl Future for Connect {
    type Output = io::Result<TcpStream>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);

        match self.result.take() {
            Some(result) => {
                Poll::Ready(result.map(|addr| TcpStream::from((self.fd.take().unwrap(), addr))))
            }
            None => Poll::Pending,
        }
    }
}
