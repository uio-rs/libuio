//! # libuio
//!
//! This is a fully featured async framework designed to run on linux and tailored for high performance
//! networking solutions. The implementation is inherently multi-threaded by design and leverages
//! `io_uring` under the hood as a I/O driver. This allows for unprecedented efficiency gains over
//! the likes of `epoll`, `poll`, and `select`. The package is split up into a handful of modules
//! each handling specifc subset of the functionality needed.
//!
//! At a high level a simple TCP echo server works as you would expect:
//!
//! ```no_run
//! use futures::StreamExt;
//!
//! use libuio::{executor::ThreadPoolBuilder, net::TcpListener};
//!
//! // First we need to create a new thread pool to execute on.
//! let pool = ThreadPoolBuilder::new()
//!     .name_prefix("executor")
//!     .create()
//!     .expect("Failed to configure thread pool.");
//!
//! // Now we spawn our main async task, which will drive any/all async operations needed by our
//! // application.
//! pool.spawn_ok(async {
//!     // Since we are demonstrating a TCP server, lets start by creating a new TcpListener that
//!     // is set to listen on [::]:9091 and have a connection backlog of 1024.
//!     let mut listener = TcpListener::with_outstanding("[::]", 9091, 1024)
//!         .expect("Failed to configure listener.");
//!
//!     let mut buf = vec![0u8; 1024];
//!
//!     // First we create an async stream of incoming connections, this is using the
//!     // opcode::AcceptMulti, which is a highly efficient implementation of the standard accept
//!     // loop. This will loop endlessly until dropped or there is an unrecoverable error.
//!     //
//!     // Note that you want to call incoming OUTSIDE of a loop like bellow, otherwise you will
//!     // be implicitly droping/recrating the incoming future which results in performance worse
//!     // than that of a a `listener.accept().await` loop would provide.
//!     let mut incoming = listener.incoming();
//!     while let Some(conn) = incoming.next().await {
//!         // We have a connection or a network error.
//!         let mut conn = match conn {
//!             Ok(conn) => conn,
//!             Err(e) => {
//!                 println!("Oh no we had an error: {}", e);
//!                 continue;
//!             }
//!         };
//!
//!         // Read some data in from the client.
//!         let (read, _) = match conn.recv(buf.as_mut_slice()).await {
//!             Ok(ret) => ret,
//!             Err(e) => {
//!                 println!("Failed to receive from client: {}", e);
//!                 continue;
//!             }
//!         };
//!
//!         // Print the data to the screen.
//!         let s = String::from_utf8_lossy(&buf[..read]);
//!         println!("Client request: {}", s);
//!
//!         // And finally echo it back to the client.
//!         conn.send(&buf[..read])
//!             .await
//!             .expect("Failed to respond to client.");
//!     }
//! });
//!
//! pool.wait();
//! ```
//!
//! As the above example demonstrates this is almost a direct drop in replacement for
//! [std::net::TcpListener] and [std::net::TcpStream].

pub mod context;
pub mod executor;
pub mod net;
pub mod uring;
