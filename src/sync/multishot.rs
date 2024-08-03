use std::{
    sync::{
        mpsc::{self, SendError, TryRecvError},
        Arc, Mutex, MutexGuard,
    },
    task::Waker,
};

#[derive(Debug)]
pub struct Receiver<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    rx: mpsc::Receiver<T>,
}

impl<T> Receiver<T> {
    fn lock_waker(&self) -> MutexGuard<'_, Option<Waker>> {
        self.waker
            .lock()
            .expect("failed to lock multishot waker: poisoned")
    }

    pub fn set_waker(&self, waker: Waker) {
        self.lock_waker().replace(waker);
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.rx.try_recv()
    }
}

#[derive(Clone, Debug)]
pub struct Sender<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    tx: mpsc::Sender<T>,
}

impl<T> Sender<T> {
    fn lock_waker(&self) -> MutexGuard<'_, Option<Waker>> {
        self.waker
            .lock()
            .expect("failed to lock multishot waker: poisoned")
    }

    pub fn push(&self, val: T) -> Result<(), SendError<T>> {
        self.tx.send(val)?;
        if let Some(waker) = self.lock_waker().take() {
            waker.wake()
        }
        Ok(())
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    let waker = Arc::new(Mutex::new(None));
    let send = Sender {
        waker: waker.clone(),
        tx,
    };
    let recv = Receiver { waker, rx };

    (send, recv)
}
