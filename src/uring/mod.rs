//! The underlying event loop implementation built on top of tokio's [io_uring] crate. This module
//! primarily exposes the [Uring] struct which is the main implementation of the event loop, this
//! also exposes a key ingredient for the implementation which is the [AsyncResult] which can be
//! used as a oneshot return value from the [Uring] to a given future that is executing async I/O.
//!
//! The implementation here is simple, the [Uring] object is injected into the various
//! implementations in [super::net] via the [super::context] implementation and thread local
//! context information. This allows the [super::net] structs to automatically register themselves
//! with the event loop, and ensure that they get polled to completion. The [Uring] is then polled
//! for I/O completion events by the [super::executor::ThreadPool] which handles pushing the
//! even loop forward.

mod error;
mod result;
mod ring;
mod token;

pub use error::{Error, Result};
pub use result::AsyncResult;
pub use ring::Uring;
pub(crate) use token::Token;
