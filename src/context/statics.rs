use std::sync::MutexGuard;

use lazy_static::lazy_static;

use crate::uring::Uring;

use super::Handle;

lazy_static! {
    static ref HANDLE: Handle = Handle::new();
}

/// Return a reference to the global [Handle] object, this contains the thread local configuration
/// needed by any/all threads in the application, namely it contains the reference to the local
/// [Uring] for use in I/O bound async tasks.
pub fn handle() -> &'static Handle {
    &HANDLE
}

/// Return a copy of the thread local [Uring], this is retrieved from the global [Handle] object
/// and is the means for injecting the IO handling into the various [crate::net] implementations.
pub fn io<'a>() -> MutexGuard<'a, Uring> {
    handle().io()
}
