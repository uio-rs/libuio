//! This module is almost a direct copy of the [futures::executor::ThreadPool],
//! [futures::executor::ThreadPoolBuilder], and the [futures::executor::unpark_mutex]
//! implementations. The reason for the copy is that we needed to implement a customized event loop
//! in order to integrate the io_uring implementation. So we took the base implementation of the
//! ThreadPool and added in the calls and logic necessary to integrate the [super::uring::Uring].
//! Otherwise the logic is identical sans the various cfg configs that were not necessary anymore.
//! Really all credit for this module should go to the developers of the [futures] crate.
//!
//! [futures]: https://docs.rs/futures/0.3.30/futures/index.html
//! [futures::executor::ThreadPool]: https://docs.rs/futures/0.3.30/futures/executor/struct.ThreadPool.html
//! [futures::executor::ThreadPoolBuilder]: https://docs.rs/futures/0.3.30/futures/executor/struct.ThreadPoolBuilder.html
//! [futures::executor::unpark_mutex]: https://github.com/rust-lang/futures-rs/blob/0.3.30/futures-executor/src/unpark_mutex.rs

mod block_on;
mod pool;
mod statics;
mod unpark_mutex;

pub use block_on::block_on;
pub use pool::{ThreadPool, ThreadPoolBuilder};
pub use statics::spawn;
