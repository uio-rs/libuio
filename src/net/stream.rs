use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use super::{Connect, Recv, Send};

/// A [TcpStream] represents a bidirectional TCP connection that can read and write data to a
/// remote host. There are two main ways to create a [TcpStream], either via the [super::TcpListener::accept]
/// and [super::TcpListener::incoming] calls, or via the [TcpStream::connect] call.
pub struct TcpStream {
    fd: OwnedFd,
    addr: SocketAddr,
}

impl TcpStream {
    /// Connect to a given remote host and return a [Connect] future to poll for completion.
    pub fn connect(addr: impl AsRef<str>, port: u16) -> io::Result<Connect> {
        let addr = format!("{}:{}", addr.as_ref(), port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        Connect::new(addr)
    }

    /// Retrieve this connected clients local [SocketAddr].
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Receive data using the given buffer from the remote host. This will return a single use
    /// [Recv] future that returns the amount of data read into the buffer, and whether or not the
    /// socket had more data available for read.
    pub fn recv<'a>(&'a mut self, buf: &'a mut [u8]) -> Recv<'a, TcpStream> {
        Recv::new(self, buf)
    }

    /// Send the data in the given buffer to the remote host. This will return a single use [Send]
    /// future that returns the amount of data sent from the buffer.
    pub fn send<'a>(&'a mut self, buf: &'a [u8]) -> Send<'a, TcpStream> {
        Send::new(self, buf)
    }
}

impl From<(OwnedFd, SocketAddr)> for TcpStream {
    fn from(tuple: (OwnedFd, SocketAddr)) -> Self {
        let (fd, addr) = tuple;
        TcpStream { fd, addr }
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
