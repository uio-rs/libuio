use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd},
};

use nix::sys::socket::{
    bind, listen, setsockopt, socket, sockopt, AddressFamily, Backlog, SockFlag, SockType,
    SockaddrStorage,
};

pub(super) fn listener_socket(addr: SocketAddr, outstanding: i32) -> io::Result<OwnedFd> {
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

    Ok(fd)
}

pub(super) fn client_socket(addr: SocketAddr) -> io::Result<OwnedFd> {
    let family = if addr.is_ipv4() {
        AddressFamily::Inet
    } else {
        AddressFamily::Inet6
    };

    socket(family, SockType::Stream, SockFlag::empty(), None).map_err(io::Error::from)
}
