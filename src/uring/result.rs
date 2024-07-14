use std::sync::{Arc, Mutex, MutexGuard};

type AsyncResultInner<T> = Arc<Mutex<Option<T>>>;

/// A single use asynchronous result handler, which wraps a Send/Sync Option of some value. It is
/// meant to be used in futures, that require a single response from the [super::Uring] I/O event
/// loop. The idea is that there are always two copies of this, one lives in the future and one
/// gets passed into the event loop. The future polls this object on every call to `poll()`, and
/// given an object exists it takes it and returns it consuming itself. The event loop will place
/// an object in here as soon as the I/O event completes and then will wake the future for
/// subsequent polling.
///
/// Note it is an error to reuse a given [AsyncResult], and in all cases should be dropped with the
/// future in question, as the event loop will *always* drop its copy after it places the result.
pub struct AsyncResult<T>(AsyncResultInner<T>);

impl<T> AsyncResult<T> {
    /// Create a new empty [AsyncResult], which is ready for use by both sides of the event loop.
    pub fn new() -> AsyncResult<T> {
        AsyncResult(Arc::new(Mutex::new(None)))
    }

    fn lock(&self) -> MutexGuard<'_, Option<T>> {
        self.0
            .lock()
            .expect("Failed to take lock on async result: poisoned")
    }

    /// Set a new value into [AsyncResult].
    pub fn set(&self, val: T) {
        self.lock().replace(val);
    }

    /// Try to pull a value out of the [AsyncResult].
    pub fn take(&self) -> Option<T> {
        self.lock().take()
    }
}

impl<T> Default for AsyncResult<T> {
    fn default() -> Self {
        AsyncResult::new()
    }
}

impl<T> Clone for AsyncResult<T> {
    fn clone(&self) -> Self {
        AsyncResult(self.0.clone())
    }
}
