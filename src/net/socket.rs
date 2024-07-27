use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd},
};

use nix::sys::socket::{
    bind, listen, setsockopt, socket, sockopt, AddressFamily, Backlog, SockFlag, SockType,
    SockaddrStorage,
};

use super::getsockname;

pub(super) fn listener_socket(
    addr: SocketAddr,
    outstanding: i32,
) -> io::Result<(OwnedFd, SocketAddr)> {
    let family = if addr.is_ipv4() {
        AddressFamily::Inet
    } else {
        AddressFamily::Inet6
    };

    let fd = socket(family, SockType::Stream, SockFlag::empty(), None)?;
    let addr = SockaddrStorage::from(addr);

    setsockopt(&fd, sockopt::ReusePort, &true)?;

    bind(fd.as_raw_fd(), &addr)?;
    listen(&fd, Backlog::new(outstanding)?)?;
    getsockname(fd.as_raw_fd()).map(|addr| (fd, addr))
}

pub(super) fn client_socket(addr: SocketAddr) -> io::Result<OwnedFd> {
    let family = if addr.is_ipv4() {
        AddressFamily::Inet
    } else {
        AddressFamily::Inet6
    };

    socket(family, SockType::Stream, SockFlag::empty(), None).map_err(io::Error::from)
}

pub(super) fn udp_socket(addr: SocketAddr) -> io::Result<OwnedFd> {
    let famil = if addr.is_ipv4() {
        AddressFamily::Inet
    } else {
        AddressFamily::Inet6
    };

    let fd = socket(famil, SockType::Datagram, SockFlag::empty(), None)?;
    let addr = SockaddrStorage::from(addr);

    setsockopt(&fd, sockopt::ReusePort, &true)?;

    bind(fd.as_raw_fd(), &addr)?;

    Ok(fd)
}
