use std::{
    sync::{Arc, Mutex, MutexGuard},
    task::Waker,
};

#[derive(Debug)]
pub struct OneShot<T> {
    result: Arc<Mutex<Option<T>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T> OneShot<T> {
    pub fn new() -> OneShot<T> {
        OneShot {
            result: Arc::new(Mutex::new(None)),
            waker: Arc::new(Mutex::new(None)),
        }
    }

    fn lock_result(&self) -> MutexGuard<'_, Option<T>> {
        self.result
            .lock()
            .expect("failed to lock oneshot result: poisoned")
    }

    fn lock_waker(&self) -> MutexGuard<'_, Option<Waker>> {
        self.waker
            .lock()
            .expect("failed to lock oneshot waker: poisoned")
    }

    pub fn complete(&self, val: T) {
        self.lock_result().replace(val);
        if let Some(waker) = self.lock_waker().take() {
            waker.wake()
        }
    }

    pub fn take(&self) -> Option<T> {
        self.lock_result().take()
    }

    pub fn set_waker(&self, waker: Waker) {
        self.lock_waker().replace(waker);
    }
}

impl<T> Default for OneShot<T> {
    fn default() -> Self {
        OneShot::new()
    }
}

impl<T> Clone for OneShot<T> {
    fn clone(&self) -> Self {
        OneShot {
            result: self.result.clone(),
            waker: self.waker.clone(),
        }
    }
}
