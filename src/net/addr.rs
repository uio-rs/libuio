use std::{
    io::{self, ErrorKind},
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    os::fd::{AsRawFd, RawFd},
};

use nix::{
    libc,
    sys::socket::{self, SockaddrStorage},
};

#[repr(C)]
pub(crate) union SocketAddrC {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrC {
    pub(crate) fn new() -> (SocketAddrC, libc::socklen_t) {
        let v6 = libc::sockaddr_in6 {
            sin6_family: 0,
            sin6_port: 0,
            sin6_flowinfo: 0,
            sin6_addr: libc::in6_addr { s6_addr: [0u8; 16] },
            sin6_scope_id: 0,
        };
        (
            SocketAddrC { v6 },
            size_of::<libc::sockaddr_in6>() as libc::socklen_t,
        )
    }
    pub(crate) fn from_std(addr: &SocketAddr) -> (SocketAddrC, libc::socklen_t) {
        match addr {
            SocketAddr::V4(ref v4) => {
                let sin_addr = libc::in_addr {
                    s_addr: u32::from_ne_bytes(v4.ip().octets()),
                };

                let sockaddr_in = libc::sockaddr_in {
                    sin_family: libc::AF_INET as libc::sa_family_t,
                    sin_port: v4.port().to_be(),
                    sin_addr,
                    sin_zero: [0u8; 8],
                };

                let sockaddr = SocketAddrC { v4: sockaddr_in };
                let socklen = size_of::<libc::sockaddr_in>() as libc::socklen_t;
                (sockaddr, socklen)
            }
            SocketAddr::V6(ref v6) => {
                let sockaddr_in6 = libc::sockaddr_in6 {
                    sin6_family: libc::AF_INET6 as libc::sa_family_t,
                    sin6_port: v6.port().to_be(),
                    sin6_addr: libc::in6_addr {
                        s6_addr: v6.ip().octets(),
                    },
                    sin6_flowinfo: v6.flowinfo(),
                    sin6_scope_id: v6.scope_id(),
                };

                let sockaddr = SocketAddrC { v6: sockaddr_in6 };
                let socklen = size_of::<libc::sockaddr_in6>() as libc::socklen_t;
                (sockaddr, socklen)
            }
        }
    }

    pub fn as_std(&self) -> SocketAddr {
        unsafe {
            match self.v4.sin_family as i32 {
                libc::AF_INET => {
                    let port = u16::from_be(self.v4.sin_port);
                    let ip = Ipv4Addr::from(self.v4.sin_addr.s_addr);

                    SocketAddr::new(IpAddr::from(ip), port)
                }
                libc::AF_INET6 => {
                    let port = u16::from_be(self.v6.sin6_port);
                    let ip = Ipv6Addr::from(self.v6.sin6_addr.s6_addr);

                    SocketAddr::new(IpAddr::from(ip), port)
                }
                _ => unreachable!(),
            }
        }
    }

    pub(crate) fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut libc::sockaddr {
        self as *mut _ as *mut libc::sockaddr
    }
}

impl From<SocketAddrC> for SocketAddr {
    fn from(value: SocketAddrC) -> SocketAddr {
        value.as_std()
    }
}

pub fn getsockname(fd: RawFd) -> io::Result<SocketAddr> {
    match socket::getsockname::<SockaddrStorage>(fd.as_raw_fd()) {
        Ok(addr) => {
            if let Some(addr) = addr.as_sockaddr_in6() {
                let addr = SocketAddr::new(IpAddr::V6(addr.ip()), addr.port());
                Ok(addr)
            } else if let Some(addr) = addr.as_sockaddr_in() {
                let addr = SocketAddr::new(IpAddr::V4(addr.ip()), addr.port());
                Ok(addr)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "got invalid peer address",
                ))
            }
        }
        Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
    }
}

pub fn getpeername(fd: RawFd) -> io::Result<SocketAddr> {
    match socket::getpeername::<SockaddrStorage>(fd) {
        Ok(addr) => {
            if let Some(addr) = addr.as_sockaddr_in6() {
                let addr = SocketAddr::new(IpAddr::V6(addr.ip()), addr.port());
                Ok(addr)
            } else if let Some(addr) = addr.as_sockaddr_in() {
                let addr = SocketAddr::new(IpAddr::V4(addr.ip()), addr.port());
                Ok(addr)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "got invalid peer address",
                ))
            }
        }
        Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
    }
}
