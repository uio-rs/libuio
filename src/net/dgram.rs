use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use super::{socket, Connect, RecvFrom, RecvMsg, SendMsg, SendTo};

/// A [UdpSocket] represents a bi-directional UDP socket that can read and write data to any remote
/// host listening for datagram messages. It is also possible to [UdpSocket::connect] to a remote
/// host in order to not have to repeatedly specify the remote address in [UdpSocket::send_to] and
/// [UdpSocket::send_msg] calls.
pub struct UdpSocket {
    fd: OwnedFd,
    addr: SocketAddr,
}

impl UdpSocket {
    /// Create a new bound [UdpSocket] ready for async communication.
    pub fn new(host: impl AsRef<str>, port: u16) -> io::Result<UdpSocket> {
        let addr = format!("{}:{}", host.as_ref(), port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let (fd, addr) = socket::udp_socket(addr)?;
        Ok(UdpSocket { fd, addr })
    }

    /// Retrieve this sockets local [SocketAddr].
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Connect to the specified remote host.
    pub fn connect(&mut self, _remote: &SocketAddr) -> Connect {
        unimplemented!()
    }

    /// Read data from the socket into the specified buffer, returning the number of bytes read and
    /// the [SocketAddr] of the remote host that sent the data.
    pub fn recv_from<'a>(&'a mut self, buf: &mut [u8]) -> RecvFrom<'a, UdpSocket> {
        RecvFrom::new(self, buf)
    }

    /// Read data from the socket into the specified buffers, returning the number of bytes read
    /// and the [SocketAddr] of the remote host that sent the data.
    pub fn recv_msg<'a>(&'a mut self, bufs: &mut [Vec<u8>]) -> RecvMsg<'a, UdpSocket> {
        RecvMsg::new(self, bufs)
    }

    /// Send the specified data to the optionally specified host. Note that on unconnected sockets
    /// the remote host is required.
    pub fn send_to<'a>(
        &'a mut self,
        buf: &mut [u8],
        addr: Option<&SocketAddr>,
    ) -> SendTo<'a, UdpSocket> {
        SendTo::new(self, buf, addr)
    }

    /// Send the data across all specified buffers to the optionally specified host. Note that on
    /// unconnected sockets the remote host is required.
    pub fn send_msg<'a>(
        &'a mut self,
        bufs: &mut [Vec<u8>],
        addr: Option<&SocketAddr>,
    ) -> SendMsg<'a, UdpSocket> {
        SendMsg::new(self, bufs, addr)
    }
}

impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
