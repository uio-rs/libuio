//! The [crate::io_uring] module represents a simplified interface ontop of the [io_uring::IoUring]
//! implementation. This module distills the implementation down to three components:
//! - The [Completion] trait which futures implement to handle async I/O results and event
//! creation.
//! - The [CompletionStatus] enum which handles informing the [UringDriver] what to do with the
//! result of a [Completion]
//! - The [UringDriver] which handles driving the async I/O and coordinating the execution with a
//! higher level executor.
//!
//! The [UringDriver] is the main async I/O event loop and is exposed via
//! [thread_local::ThreadLocal] types in the [crate::context] module. It is generally unneeded to
//! create instances of a [UringDriver] directly.

mod cancel;
mod completion;
mod engine;

pub use completion::{Completion, CompletionStatus};
pub use engine::UringDriver;
