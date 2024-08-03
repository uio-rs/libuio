use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use crate::net::{
    getpeername, getsockname, socket, Connect, Recv, RecvFrom, RecvMsg, Send, SendMsg, SendTo,
};

/// A [UdpSocket] represents a bi-directional UDP socket that can read and write data to any remote
/// host listening for datagram messages. It is also possible to [UdpSocket::connect] to a remote
/// host in order to not have to repeatedly specify the remote address in [UdpSocket::send_to] and
/// [UdpSocket::send_msg] calls.
pub struct UdpSocket {
    fd: OwnedFd,
}

impl UdpSocket {
    /// Create a new bound [UdpSocket] ready for async communication.
    pub fn new(host: impl AsRef<str>, port: u16) -> io::Result<UdpSocket> {
        let addr = format!("{}:{}", host.as_ref(), port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let fd = socket::udp_socket(addr)?;
        Ok(UdpSocket { fd })
    }

    /// Retrieve this sockets local [SocketAddr], or panics if there is either no local address or
    /// some other [std::io::Error] is encountered.
    ///
    /// For a safe alternative use [UdpSocket::try_local_addr].
    pub fn local_addr(&self) -> SocketAddr {
        self.try_local_addr().unwrap()
    }

    /// Retrieve this sockets local [SocketAddr] or returns an error if there is either no local
    /// address for this socket or some other [std::io::Error] is encountered.
    pub fn try_local_addr(&self) -> io::Result<SocketAddr> {
        getsockname(self.fd.as_raw_fd())
    }

    /// Retrieve the peer [SocketAddr] for connected socket which have successfully called
    /// [UdpSocket::connect]. or panics an error if there is either no peer address or some other
    /// [std::io::Error] is encountered.
    ///
    /// For a safe alternative use [UdpSocket::try_peer_addr].
    pub fn peer_addr(&self) -> SocketAddr {
        self.try_peer_addr().unwrap()
    }

    /// Retrieve the peer [SocketAddr] for connected socket which have successfully called
    /// [UdpSocket::connect]. or returns an error if there is either no peer address or some other
    /// [std::io::Error] is encountered.
    pub fn try_peer_addr(&self) -> io::Result<SocketAddr> {
        getpeername(self.fd.as_raw_fd())
    }

    /// Connect to the specified remote host.
    pub fn connect<'a>(&'a mut self, remote: &SocketAddr) -> Connect<'a, UdpSocket> {
        Connect::new(self, remote)
    }

    /// Read data from the socket into the specified buffer, returning the number of bytes read.
    /// Note that this method requires that [UdpSocket::connect] be called successfuly to set the
    /// remote address.
    pub fn recv(&mut self, buf: Vec<u8>) -> Recv<'_, UdpSocket> {
        Recv::new(self, buf)
    }

    /// Read data from the socket into the specified buffer, returning the number of bytes read and
    /// the [SocketAddr] of the remote host that sent the data.
    pub fn recv_from(&mut self, buf: Vec<u8>) -> RecvFrom<'_, UdpSocket> {
        RecvFrom::new(self, buf)
    }

    /// Read data from the socket into the specified buffers, returning the number of bytes read
    /// and the [SocketAddr] of the remote host that sent the data.
    pub fn recv_msg(&mut self, bufs: Vec<Vec<u8>>) -> RecvMsg<'_, UdpSocket> {
        RecvMsg::new(self, bufs)
    }

    /// Send the specified data to the remote peer, returning the number of bytes read. Note that
    /// this method requires that [UdpSocket::connect] be called successfuly to set the remote
    /// address.
    pub fn send(&mut self, buf: Vec<u8>) -> Send<'_, UdpSocket> {
        Send::new(self, buf)
    }

    /// Send the specified data to the optionally specified host. Note that on unconnected sockets
    /// the remote host is required.
    pub fn send_to(&mut self, buf: Vec<u8>, addr: Option<SocketAddr>) -> SendTo<'_, UdpSocket> {
        SendTo::new(self, buf, addr)
    }

    /// Send the data across all specified buffers to the optionally specified host. Note that on
    /// unconnected sockets the remote host is required.
    pub fn send_msg(
        &mut self,
        bufs: Vec<Vec<u8>>,
        addr: Option<SocketAddr>,
    ) -> SendMsg<'_, UdpSocket> {
        SendMsg::new(self, bufs, addr)
    }
}

impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
