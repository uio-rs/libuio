use std::{
    io,
    marker::PhantomData,
    net::SocketAddr,
    os::fd::{AsRawFd, OwnedFd},
    pin::Pin,
    sync::mpsc::{Receiver, TryRecvError},
    task::{Context, Poll},
};

use futures::{Future, Stream};

use crate::{context, uring};

use super::{socket, TcpStream};

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
        let fd = socket::listener_socket(addr, outstanding)?;

        Ok(TcpListener { addr, fd })
    }

    /// Return the address this listener is bound to.
    pub fn address(&self) -> &SocketAddr {
        &self.addr
    }

    /// Accept a single connection asynchronously, this will return an [Accept] future that when
    /// polled to completion will either return a valid [TcpStream] that is ready to use or an
    /// [std::io::Error] describing any errors that might have occured.
    pub fn accept(&mut self) -> Accept {
        Accept::new(self)
    }

    /// Accept a stream of incoming connections, this will return an [Incoming] future [Stream]
    /// that when iterated on will return valid [TcpStream] objects or [std::io::Error] objects
    /// describing issues enountered.
    ///
    /// Note that its best to call this outside of a loop body or conditional, as the future is
    /// meant to be reused.
    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming::new(self)
    }
}

/// This represents a single use future for accepting an active conntion from a live [TcpListener].
/// When polled to completion the future will return a valid [TcpStream], or any [std::io::Error]
/// encountered while awaiting the new connection.
pub struct Accept<'a> {
    inner: PhantomData<&'a mut TcpListener>,
    id: usize,
    result: uring::AsyncResult<uring::Result<OwnedFd>>,
}

impl<'a> Drop for Accept<'a> {
    fn drop(&mut self) {
        context::io().deregister(self.id);
    }
}

impl<'a> Accept<'a> {
    pub fn new(listener: &'a mut TcpListener) -> Accept<'a> {
        let (result, id) = context::io().register_accept(listener.fd.as_raw_fd());

        Accept {
            inner: PhantomData,
            id,
            result,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        context::io().set_waker(self.id, cx.waker().clone());
    }
}

impl<'a> Future for Accept<'a> {
    type Output = uring::Result<TcpStream>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.set_waker(cx);
        match self.result.take() {
            Some(result) => Poll::Ready(result.map(TcpStream::from)),
            None => Poll::Pending,
        }
    }
}

/// This represents a stream future of incoming [TcpStream] connections. This will continue to
/// return connections until either the future is dropped, or there is an unrecoverable error
/// enountered.
///
/// Note this future is meant to be reused, so ensure that when in use that its lifetime extends
/// beyond any loops in use.
pub struct Incoming<'a> {
    inner: PhantomData<&'a mut TcpListener>,
    id: usize,
    stream: Receiver<uring::Result<OwnedFd>>,
}

impl<'a> Drop for Incoming<'a> {
    fn drop(&mut self) {
        context::io().deregister(self.id);
    }
}

impl<'a> Incoming<'a> {
    fn new(listener: &'a mut TcpListener) -> Incoming<'a> {
        let (stream, id) = context::io().register_incoming(listener.fd.as_raw_fd());
        Incoming {
            inner: PhantomData,
            id,
            stream,
        }
    }

    fn set_waker(&mut self, cx: &mut Context<'_>) {
        context::io().set_waker(self.id, cx.waker().clone());
    }
}

impl<'a> Stream for Incoming<'a> {
    type Item = uring::Result<TcpStream>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.set_waker(cx);
        match self.stream.try_recv().map(Some) {
            Ok(val) => match val {
                Some(val) => Poll::Ready(Some(val.map(TcpStream::from))),
                None => Poll::Ready(None),
            },
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(_) => panic!("Fuck me something went horribly wrong."),
        }
    }
}
