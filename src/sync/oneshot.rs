use std::{
    sync::{Arc, Mutex, MutexGuard},
    task::Waker,
};

#[derive(Debug)]
enum OneShotInner<T> {
    Pending,
    Complete(T),
    Finalized,
}

impl<T> OneShotInner<T> {
    pub fn new() -> OneShotInner<T> {
        OneShotInner::Pending
    }

    pub fn complete(&mut self, val: T) {
        use OneShotInner::*;
        match self {
            Finalized => panic!("invalid state can not call complete on finalized one shot."),
            Complete(..) => panic!("invalid state can not call complete more than once."),
            _ => *self = Complete(val),
        };
    }

    fn unwrap(self) -> T {
        use OneShotInner::*;
        match self {
            Pending | Finalized => unreachable!("unwrap called on pending/finalized OneShot"),
            Complete(val) => val,
        }
    }

    pub fn take(&mut self) -> Option<T> {
        use OneShotInner::*;
        match self {
            Pending | Finalized => None,
            _ => Some(std::mem::replace(self, OneShotInner::Finalized).unwrap()),
        }
    }
}

#[derive(Debug)]
pub struct OneShot<T> {
    inner: Arc<Mutex<OneShotInner<T>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T> OneShot<T> {
    pub fn new() -> OneShot<T> {
        OneShot {
            inner: Arc::new(Mutex::new(OneShotInner::new())),
            waker: Arc::new(Mutex::new(None)),
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, OneShotInner<T>> {
        self.inner
            .lock()
            .expect("failed to lock oneshot result: poisoned")
    }

    fn lock_waker(&self) -> MutexGuard<'_, Option<Waker>> {
        self.waker
            .lock()
            .expect("failed to lock oneshot waker: poisoned")
    }

    pub fn complete(&self, val: T) {
        self.lock_inner().complete(val);
        if let Some(waker) = self.lock_waker().take() {
            waker.wake()
        }
    }

    pub fn take(&self) -> Option<T> {
        self.lock_inner().take()
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
            inner: self.inner.clone(),
            waker: self.waker.clone(),
        }
    }
}
