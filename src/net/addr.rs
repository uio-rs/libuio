use std::{mem::size_of, net::SocketAddr};

use nix::libc;

#[repr(C)]
pub(crate) union SocketAddrC {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrC {
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

    pub(crate) fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}
