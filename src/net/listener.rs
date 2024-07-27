use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use super::{socket, Accept, Incoming};

const DEFAULT_OUSTANDING: i32 = 1024;

/// A [TcpListener] represents a io_uring based TCP listener socket to accept incoming connections on,
/// this listener is capable of sharing a single socket address across multiple listeners, via the
/// SO_REUSEPORT socket option.
///
/// There are two main ways of consuming the [TcpListener]. You can use the [TcpListener::accept]
/// call which will return a single use [Accept] future, which when polled will return a single
/// connection that is ready to use. Or you can use the [TcpListener::incoming] call which will
/// return a [Stream] object in the form of an [Incoming] future which you can iterate over.
///
/// # Examples
///
/// ```no_run
/// # use std::io;
/// # use futures::stream::StreamExt;
/// # use libuio::net::TcpListener;
/// # use libuio::executor::ThreadPoolBuilder;
/// # fn main() -> io::Result<()> {
/// # let pool = ThreadPoolBuilder::new()
/// #   .name_prefix("executor")
/// #   .create()
/// #   .expect("Failed to configure thread pool.");
/// # pool.spawn_ok(async {
/// let mut listener = TcpListener::new("[::]", 9092).expect("Failed to listen on specified address.");
///
/// let mut incoming = listener.incoming();
/// while let Some(connection) = incoming.next().await {
///     let connection = connection.expect("Failed to accept connection.");
///     // Do something with the connection.
/// }
/// # });
/// # Ok(())
/// # }
/// ```
pub struct TcpListener {
    addr: SocketAddr,
    fd: OwnedFd,
}

impl TcpListener {
    /// Create a new [TcpListener] and listen on the specified address and port combination, using
    /// the default outstanding connections setting.
    pub fn new(host: impl AsRef<str>, port: u16) -> io::Result<TcpListener> {
        Self::with_outstanding(host, port, DEFAULT_OUSTANDING)
    }

    /// Create a new [TcpListener] like [TcpListener::new], however allow overriding the outstanding
    /// connection queue size.
    pub fn with_outstanding(
        host: impl AsRef<str>,
        port: u16,
        outstanding: i32,
    ) -> io::Result<TcpListener> {
        let addr = format!("{}:{}", host.as_ref(), port).parse().unwrap();
        let (fd, addr) = socket::listener_socket(addr, outstanding)?;

        Ok(TcpListener { addr, fd })
    }

    /// Return the address this listener is bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Accept a single connection asynchronously, this will return an [Accept] future that when
    /// polled to completion will either return a valid [TcpStream] that is ready to use or an
    /// [std::io::Error] describing any errors that might have occured.
    pub fn accept(&mut self) -> Accept<'_, TcpListener> {
        Accept::new(self)
    }

    /// Accept a stream of incoming connections, this will return an [Incoming] future [Stream]
    /// that when iterated on will return valid [TcpStream] objects or [std::io::Error] objects
    /// describing issues enountered.
    ///
    /// Note that its best to call this outside of a loop body or conditional, as the future is
    /// meant to be reused.
    pub fn incoming(&mut self) -> Incoming<'_, TcpListener> {
        Incoming::new(self)
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
