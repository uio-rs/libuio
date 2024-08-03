use std::sync::{Arc, Mutex, MutexGuard};

use thread_local::ThreadLocal;

use super::UringDriver;

/// Represents a thread local handle to retrieve a [Uring] object from. This is used to
/// transparently inject the [Uring] into the various [crate::net] implementations. It is generally
/// not a good idea to create this manually, in fact you should cannot do so directly and instead leverage
/// either [super::statics::handle] or [super::statics::io] to get access to one of these.
#[derive(Clone)]
pub struct Handle {
    uring_driver: Arc<ThreadLocal<Mutex<UringDriver>>>,
}

impl Handle {
    pub(super) fn new() -> Handle {
        Handle {
            uring_driver: Arc::new(ThreadLocal::new()),
        }
    }

    pub fn uring(&self) -> MutexGuard<'_, UringDriver> {
        self.uring_driver
            .get_or(|| {
                UringDriver::new(4096)
                    .map(Mutex::new)
                    .expect("Failed to configure the UringDriver.")
            })
            .lock()
            .expect("Failed to lock thread local UringDriver: poisoned")
    }
}
