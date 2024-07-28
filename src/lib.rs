//! # libuio
//!
//! This is a fully featured async framework designed to run on linux and tailored for high performance
//! networking solutions. The implementation is inherently multi-threaded by design and leverages
//! `io_uring` under the hood as a I/O driver. This allows for unprecedented efficiency gains over
//! the likes of `epoll`, `poll`, and `select`. The package is split up into a handful of modules
//! each handling specifc subset of the functionality needed.
//!
//! For detailed examples see the [examples](https://github.com/uio-rs/libuio/tree/main/examples) directory in the root of this repository.
//!
//! At a high level a simple TCP echo server works as you would expect:
//!
//! ```no_run
//! use std::io;
//!
//! use futures::StreamExt;
//!
//! use libuio::net::TcpListener;
//!
//! #[libuio::main]
//! async fn main() -> io::Result<()> {
//!     // Since we are demonstrating a TCP server, lets start by creating a new TcpListener that
//!     // is set to listen on [::]:9091 and have a connection backlog of 1024.
//!     let mut listener = TcpListener::with_outstanding("[::]", 9091, 1024)?;
//!
//!     let mut buf = vec![0u8; 1024];
//!
//!     println!("Listening on: {}", listener.addr());
//!
//!     // Or we can grab a async stream of incoming connections, this is using the
//!     // opcode::AcceptMulti, which is a highly efficient implementation of the standard accept
//!     // loop. This will loop endlessly until dropped or there is an unrecoverable error.
//!     //
//!     // Note that you want to call incoming OUTSIDE of a loop like bellow, otherwise you will
//!     // be implicitly droping/recrating the incoming future which results in performance worse
//!     // than that of a a `listener.accept().await` loop would provide.
//!     let mut incoming = listener.incoming();
//!     while let Some(conn) = incoming.next().await {
//!         let mut conn = match conn {
//!             Ok(conn) => conn,
//!             Err(e) => {
//!                 println!("Oh no we had an error: {}", e);
//!                 continue;
//!             }
//!         };
//!
//!         println!("Got connection from: {}", conn.addr());
//!
//!         let read = match conn.recv(buf.as_mut_slice()).await {
//!             Ok(ret) => ret,
//!             Err(e) => {
//!                 println!("Failed to receive from client: {}", e);
//!                 continue;
//!             }
//!         };
//!
//!         let s = String::from_utf8_lossy(&buf[..read]);
//!
//!         println!("Client request: {}", s);
//!
//!         conn.send(&buf[..read])
//!             .await
//!             .expect("Failed to respond to client.");
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Similarly here is an example TCP client interacting with the above server:
//!
//! ```no_run
//! use std::io;
//!
//! use libuio::net::TcpStream;
//!
//! #[libuio::main]
//! async fn main() -> io::Result<()> {
//!     println!("Connecting to remote server.");
//!
//!     let mut client = TcpStream::connect("[::1]", 9091)?.await?;
//!
//!     println!(
//!         "Connected to remote server, local address: {}",
//!         client.addr()
//!     );
//!
//!     client.send("Hello from client!".as_bytes()).await?;
//!
//!     let mut buf = vec![0u8; 1024];
//!     let read = client.recv(buf.as_mut_slice()).await?;
//!
//!     let str = String::from_utf8_lossy(&buf[..read]);
//!     println!("Server response: {}", str);
//!     Ok(())
//! }
//! ```
//!
//! As the above example demonstrates this is almost a direct drop in replacement for
//! [std::net::TcpListener] and [std::net::TcpStream].

pub mod context;
pub mod executor;
pub mod io_uring;
pub mod net;
pub(crate) mod ptr;
pub mod sync;

pub use executor::{spawn, ThreadPool, ThreadPoolBuilder};
pub use libuio_macros::main;
