//! The [self] package handles all logic relating to creating and managing network IO objects.
//! Namely this exposes a set of socket implementations to support networking applications and aims
//! to be a drop in replacement for [std::net] or [tokio::net] modules.
//!
//! This module primarily exposes the following objects:
//! - [TcpListener] which represents an async TCP listener socket.
//! - [TcpStream] which represnets an async bi-directional stream socket.
//! - [UdpSocket] which represents an async bi-directional datagram socket.
//!
//! These implementations all leverage [io_uring] under the hood to power their async I/O
//! implementations this means that these are highly efficient and leverage the latest and greatest
//! in linux networking technologies. However this also means that much of this work, as mentioned
//! elsewhere, is locked in the newest kernels really targeting v6.x+ only.
//!
//! [tokio::net]: https://docs.rs/tokio/latest/tokio/net/index.html

mod futures;
mod socket;
mod tcp;
mod types;
mod udp;

pub(crate) use types::*;

pub use futures::*;
pub use tcp::{TcpListener, TcpStream};
pub use udp::UdpSocket;

