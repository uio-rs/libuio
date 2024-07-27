//! The context module handles the logic for sharing local thread context between tasks and objects
//! that need that context. Namely it exposes a [Handle] object as a application static global,
//! this handle stores [thread_local::ThreadLocal] objects that can be injected as needed into
//! logic throughout the application. There are two main means of accessing the thread context
//! either accessing the top level [Handle] via the [statics::handle] method or using the helper
//! method [statics::io] which returns a reference to the local [crate::uring::Uring] directly.
//!
//! Generally speaking you should NOT be creating [Handle] objects directly and instead should
//! leverage the above helpers to do so.

mod handle;
mod statics;

pub use handle::Handle;
pub use statics::{handle, uring};
