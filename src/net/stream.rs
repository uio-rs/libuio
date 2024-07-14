use std::{
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;

use crate::{
    context,
    net::{socket, SocketAddrC},
    uring,
};

/// A [TcpStream] represents a bidirectional TCP connection that can read and write data to a
/// remote host. There are two main ways to create a [TcpStream], either via the [super::TcpListener::accept]
/// and [super::TcpListener::incoming] calls, or via the [TcpStream::connect] call.
pub struct TcpStream {
    fd: OwnedFd,
}

impl TcpStream {
    /// Connect to a given remote host and return a [Connect] future to poll for completion.
    pub fn connect(addr: impl AsRef<str>, port: u16) -> io::Result<Connect> {
        let addr = format!("{}:{}", addr.as_ref(), port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        Connect::new(addr)
    }

    /// Receive data using the given buffer from the remote host. This will return a single use
    /// [Recv] future that returns the amount of data read into the buffer, and whether or not the
    /// socket had more data available for read.
    pub fn recv<'a>(&'a mut self, buf: &'a mut [u8]) -> Recv<'a> {
        Recv::new(self, buf)
    }

    /// Send the data in the given buffer to the remote host. This will return a single use [Send]
    /// future that returns the amount of data sent from the buffer.
    pub fn send<'a>(&'a mut self, buf: &'a [u8]) -> Send<'a> {
        Send::new(self, buf)
    }
}

impl From<OwnedFd> for TcpStream {
    fn from(fd: OwnedFd) -> Self {
        TcpStream { fd }
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

/// This represents a single use asynchronous receive on a connected [TcpStream], it will use the
/// given buffer to read data into, and ultimately return the amount of data read and whether or
/// not ther was still data in the socket after the receive completed.
pub struct Recv<'a> {
    inner: PhantomData<&'a mut TcpStream>,
    id: usize,
    result: uring::AsyncResult<uring::Result<(usize, bool)>>,
}

impl<'a> Drop for Recv<'a> {
    fn drop(&mut self) {
        context::io().deregister(self.id);
    }
}

impl<'a> Recv<'a> {
    pub fn new(stream: &'a mut TcpStream, buf: &'a mut [u8]) -> Recv<'a> {
        let (result, id) = context::io().register_recv(stream.fd.as_raw_fd(), buf);
        Recv {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        context::io().set_waker(self.id, cx.waker().clone());
    }
}

impl<'a> Future for Recv<'a> {
    type Output = uring::Result<(usize, bool)>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}

/// This represents a single use asynchronous send operation on a connected [TcpStream], it will
/// use the given buffer to write data from, and ultimately return the amount of data written to
/// the remote server.
pub struct Send<'a> {
    inner: PhantomData<&'a mut TcpStream>,
    id: usize,
    result: uring::AsyncResult<uring::Result<usize>>,
}

impl<'a> Drop for Send<'a> {
    fn drop(&mut self) {
        context::io().deregister(self.id);
    }
}

impl<'a> Send<'a> {
    pub fn new(stream: &'a mut TcpStream, buf: &'a [u8]) -> Send<'a> {
        let (result, id) = context::io().register_send(stream.fd.as_raw_fd(), buf);
        Send {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        context::io().set_waker(self.id, cx.waker().clone());
    }
}

impl<'a> Future for Send<'a> {
    type Output = uring::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}

/// This represents a single use asynchronous connect operation to create a new [TcpStream] object
/// to interact with a remote host on. This will ultimately return the connected and ready to use
/// [TcpStream].
pub struct Connect {
    addr: Option<Pin<Box<SocketAddrC>>>,
    fd: Option<OwnedFd>,
    id: usize,
    result: uring::AsyncResult<uring::Result<()>>,
}

impl Drop for Connect {
    fn drop(&mut self) {
        context::io().deregister(self.id);
    }
}

impl Connect {
    pub fn new(addr: SocketAddr) -> io::Result<Connect> {
        let fd = socket::client_socket(addr)?;
        let (addr, len) = SocketAddrC::from_std(&addr);
        let addr = Box::pin(addr);
        let (result, id) = context::io().register_connect(fd.as_raw_fd(), &addr, len);
        Ok(Connect {
            addr: Some(addr), // Just need this to live until Connect is dropped.
            fd: Some(fd),
            id,
            result,
        })
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        context::io().set_waker(self.id, cx.waker().clone());
    }
}

impl Future for Connect {
    type Output = uring::Result<TcpStream>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);

        match self.result.take() {
            Some(result) => {
                self.addr.take().unwrap();
                Poll::Ready(result.map(|_| TcpStream::from(self.fd.take().unwrap())))
            }
            None => Poll::Pending,
        }
    }
}
