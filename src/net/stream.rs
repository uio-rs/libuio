use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use super::{getpeername, getsockname, socket, Connect, Recv, Send};

/// A [TcpStream] represents a bidirectional TCP connection that can read and write data to a
/// remote host. There are two main ways to create a [TcpStream], either via the [super::TcpListener::accept]
/// and [super::TcpListener::incoming] calls, or via the [TcpStream::connect] call.
pub struct TcpStream {
    fd: OwnedFd,
}

impl TcpStream {
    pub fn new(ipv4: bool) -> io::Result<TcpStream> {
        socket::client_socket(ipv4).map(TcpStream::from)
    }

    /// Connect to a given remote host and return a [Connect] future to poll for completion.
    pub fn connect<'a>(&'a mut self, addr: &SocketAddr) -> Connect<'a, TcpStream> {
        Connect::new(self, addr)
    }

    /// Retrieve this sockets local [SocketAddr], or panics if there is either no local address or
    /// some other [std::io::Error] is encountered.
    ///
    /// For a safe alternative use [TcpStream::try_local_addr].
    pub fn local_addr(&self) -> SocketAddr {
        self.try_local_addr().unwrap()
    }

    /// Retrieve this sockets local [SocketAddr] or returns an error if there is either no local
    /// address for this socket or some other [std::io::Error] is encountered.
    pub fn try_local_addr(&self) -> io::Result<SocketAddr> {
        getsockname(self.fd.as_raw_fd())
    }

    /// Retrieve the peer [SocketAddr] for connected socket which have successfully called
    /// [TcpStream::connect]. or panics an error if there is either no peer address or some other
    /// [std::io::Error] is encountered.
    ///
    /// For a safe alternative use [TcpStream::try_peer_addr].
    pub fn peer_addr(&self) -> SocketAddr {
        self.try_peer_addr().unwrap()
    }

    /// Retrieve the peer [SocketAddr] for connected socket which have successfully called
    /// [TcpStream::connect]. or returns an error if there is either no peer address or some other
    /// [std::io::Error] is encountered.
    pub fn try_peer_addr(&self) -> io::Result<SocketAddr> {
        getpeername(self.fd.as_raw_fd())
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
