use std::sync::{Arc, Mutex, MutexGuard};

use thread_local::ThreadLocal;

use crate::uring::Uring;

/// Represents a thread local handle to retrieve a [Uring] object from. This is used to
/// transparently inject the [Uring] into the various [crate::net] implementations. It is generally
/// not a good idea to create this manually, in fact you should cannot do so directly and instead leverage
/// either [super::statics::handle] or [super::statics::io] to get access to one of these.
#[derive(Clone)]
pub struct Handle {
    io: Arc<ThreadLocal<Mutex<Uring>>>,
}

impl Handle {
    pub(super) fn new() -> Handle {
        Handle {
            io: Arc::new(ThreadLocal::new()),
        }
    }

    pub fn io(&self) -> MutexGuard<'_, Uring> {
        self.io
            .get_or(|| {
                Uring::new(4096)
                    .map(Mutex::new)
                    .expect("Failed to configure thread local io_uring.")
            })
            .lock()
            .expect("Failed to take I/O driver lock: poisoned.")
    }
}
