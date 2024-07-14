use std::{
    boxed::Box,
    cmp, fmt, io,
    sync::atomic::{AtomicUsize, Ordering},
    sync::mpsc::{self, TryRecvError},
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_utils::sync::WaitGroup;
use futures::{
    executor::enter,
    future::{Future, FutureExt, FutureObj},
    task::{waker_ref, ArcWake, Context, Poll, Spawn, SpawnError},
};

use crate::context;

use super::unpark_mutex::UnparkMutex;

/// This is a modified version of the [futures::executor::ThreadPool],
/// that integrates an io_uring based I/O completion system into it. Otherwise the implementation
/// is identical and all rights and credit should go to the original developers.
///
/// [futures::executor::ThreadPool]: https://docs.rs/futures/latest/futures/executor/struct.ThreadPool.html
pub struct ThreadPool {
    state: Arc<PoolState>,
    wg: Option<WaitGroup>,
}

/// Thread pool configuration object.
pub struct ThreadPoolBuilder {
    pool_size: usize,
    stack_size: usize,
    name_prefix: Option<String>,
    after_start: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    before_stop: Option<Arc<dyn Fn(usize) + Send + Sync>>,
}

#[allow(dead_code)]
trait AssertSendSync: Send + Sync {}
impl AssertSendSync for ThreadPool {}

struct PoolState {
    tx: Mutex<mpsc::Sender<Message>>,
    rx: Mutex<mpsc::Receiver<Message>>,
    cnt: AtomicUsize,
    size: usize,
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadPool")
            .field("size", &self.state.size)
            .finish()
    }
}

impl fmt::Debug for ThreadPoolBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadPoolBuilder")
            .field("pool_size", &self.pool_size)
            .field("name_prefix", &self.name_prefix)
            .finish()
    }
}

enum Message {
    Run(Task),
    Close,
}

impl ThreadPool {
    /// Creates a new thread pool with the default configuration.
    ///
    /// See documentation for the methods in
    /// [`ThreadPoolBuilder`](ThreadPoolBuilder) for details on the default
    /// configuration.
    pub fn new() -> Result<Self, io::Error> {
        ThreadPoolBuilder::new().create()
    }

    /// Create a default thread pool configuration, which can then be customized.
    ///
    /// See documentation for the methods in
    /// [`ThreadPoolBuilder`](ThreadPoolBuilder) for details on the default
    /// configuration.
    pub fn builder() -> ThreadPoolBuilder {
        ThreadPoolBuilder::new()
    }

    /// Spawns a future that will be run to completion.
    ///
    /// > **Note**: This method is similar to `Spawn::spawn_obj`, except that
    /// >           it is guaranteed to always succeed.
    pub fn spawn_obj_ok(&self, future: FutureObj<'static, ()>) {
        let task = Task {
            future,
            wake_handle: Arc::new(WakeHandle {
                exec: self.clone(),
                mutex: UnparkMutex::new(),
            }),
            exec: self.clone(),
        };
        self.state.send(Message::Run(task));
    }

    /// Spawns a task that polls the given future with output `()` to
    /// completion.
    ///
    /// ```
    /// # {
    /// use libuio::executor::ThreadPool;
    ///
    /// let pool = ThreadPool::new().unwrap();
    ///
    /// let future = async { /* ... */ };
    /// pool.spawn_ok(future);
    /// # }
    /// # std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
    /// ```
    ///
    /// > **Note**: This method is similar to `SpawnExt::spawn`, except that
    /// >           it is guaranteed to always succeed.
    pub fn spawn_ok<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.spawn_obj_ok(FutureObj::new(Box::new(future)))
    }

    /// Wait for the [ThreadPool] to exit completely and shutdown.
    pub fn wait(mut self) {
        if let Some(wg) = self.wg.take() {
            wg.wait();
        }
    }
}

impl Spawn for ThreadPool {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        self.spawn_obj_ok(future);
        Ok(())
    }
}

impl PoolState {
    fn send(&self, msg: Message) {
        self.tx.lock().unwrap().send(msg).unwrap();
    }

    fn handle_tasks(&self) -> bool {
        loop {
            // Now grab any ready tasks and execute them.
            let msg = match self.rx.lock().unwrap().try_recv() {
                Ok(msg) => msg,                                 // We got a task to execute or close.
                Err(TryRecvError::Empty) => return false,       // Nothing ready just continue on.
                Err(TryRecvError::Disconnected) => return true, // Something horrible happened, shutdown.
            };

            // Handle our message and then loop again.
            match msg {
                Message::Run(task) => task.run(),
                Message::Close => return true,
            }
        }
    }

    fn work(
        &self,
        idx: usize,
        after_start: Option<Arc<dyn Fn(usize) + Send + Sync>>,
        before_stop: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    ) {
        let _scope = enter().unwrap();
        if let Some(after_start) = after_start {
            after_start(idx);
        }
        loop {
            // Grab our thread local io_uring and run it.
            context::io().run().expect("Failed to run I/O loop.");

            // Now handle any outstanding tasks, breaking out of the loop if we are in graceful
            // shutdown mode or we had a fatal error.
            if self.handle_tasks() {
                break;
            }
        }
        if let Some(before_stop) = before_stop {
            before_stop(idx);
        }
    }
}

impl Clone for ThreadPool {
    fn clone(&self) -> Self {
        self.state.cnt.fetch_add(1, Ordering::Relaxed);
        Self {
            state: self.state.clone(),
            wg: self.wg.clone(),
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        if self.state.cnt.fetch_sub(1, Ordering::Relaxed) == 1 {
            for _ in 0..self.state.size {
                self.state.send(Message::Close);
            }
        }
    }
}

impl ThreadPoolBuilder {
    /// Create a default thread pool configuration.
    ///
    /// See the other methods on this type for details on the defaults.
    pub fn new() -> Self {
        Self {
            pool_size: cmp::max(1, num_cpus::get()),
            stack_size: 0,
            name_prefix: None,
            after_start: None,
            before_stop: None,
        }
    }

    /// Set size of a future ThreadPool
    ///
    /// The size of a thread pool is the number of worker threads spawned. By
    /// default, this is equal to the number of CPU cores.
    ///
    /// # Panics
    ///
    /// Panics if `pool_size == 0`.
    pub fn pool_size(&mut self, size: usize) -> &mut Self {
        assert!(size > 0);
        self.pool_size = size;
        self
    }

    /// Set stack size of threads in the pool, in bytes.
    ///
    /// By default, worker threads use Rust's standard stack size.
    pub fn stack_size(&mut self, stack_size: usize) -> &mut Self {
        self.stack_size = stack_size;
        self
    }

    /// Set thread name prefix of a future ThreadPool.
    ///
    /// Thread name prefix is used for generating thread names. For example, if prefix is
    /// `my-pool-`, then threads in the pool will get names like `my-pool-1` etc.
    ///
    /// By default, worker threads are assigned Rust's standard thread name.
    pub fn name_prefix<S: Into<String>>(&mut self, name_prefix: S) -> &mut Self {
        self.name_prefix = Some(name_prefix.into());
        self
    }

    /// Execute the closure `f` immediately after each worker thread is started,
    /// but before running any tasks on it.
    ///
    /// This hook is intended for bookkeeping and monitoring.
    /// The closure `f` will be dropped after the `builder` is dropped
    /// and all worker threads in the pool have executed it.
    ///
    /// The closure provided will receive an index corresponding to the worker
    /// thread it's running on.
    pub fn after_start<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        self.after_start = Some(Arc::new(f));
        self
    }

    /// Execute closure `f` just prior to shutting down each worker thread.
    ///
    /// This hook is intended for bookkeeping and monitoring.
    /// The closure `f` will be dropped after the `builder` is dropped
    /// and all threads in the pool have executed it.
    ///
    /// The closure provided will receive an index corresponding to the worker
    /// thread it's running on.
    pub fn before_stop<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        self.before_stop = Some(Arc::new(f));
        self
    }

    /// Create a [`ThreadPool`](ThreadPool) with the given configuration.
    pub fn create(&mut self) -> Result<ThreadPool, io::Error> {
        let (tx, rx) = mpsc::channel();
        let wg = WaitGroup::new();
        let mut pool = ThreadPool {
            state: Arc::new(PoolState {
                tx: Mutex::new(tx),
                rx: Mutex::new(rx),
                cnt: AtomicUsize::new(1),
                size: self.pool_size,
            }),
            wg: None,
        };

        for counter in 0..self.pool_size {
            let state = pool.state.clone();
            let wg = wg.clone();
            let after_start = self.after_start.clone();
            let before_stop = self.before_stop.clone();
            let mut thread_builder = thread::Builder::new();
            if let Some(ref name_prefix) = self.name_prefix {
                thread_builder = thread_builder.name(format!("{}{}", name_prefix, counter));
            }
            if self.stack_size > 0 {
                thread_builder = thread_builder.stack_size(self.stack_size);
            }
            thread_builder.spawn(move || {
                state.work(counter, after_start, before_stop);
                drop(wg)
            })?;
        }
        pool.wg = Some(wg);
        Ok(pool)
    }
}

impl Default for ThreadPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A task responsible for polling a future to completion.
struct Task {
    future: FutureObj<'static, ()>,
    exec: ThreadPool,
    wake_handle: Arc<WakeHandle>,
}

struct WakeHandle {
    mutex: UnparkMutex<Task>,
    exec: ThreadPool,
}

impl Task {
    /// Actually run the task (invoking `poll` on the future) on the current
    /// thread.
    fn run(self) {
        let Self {
            mut future,
            wake_handle,
            mut exec,
        } = self;
        let waker = waker_ref(&wake_handle);
        let mut cx = Context::from_waker(&waker);

        // Safety: The ownership of this `Task` object is evidence that
        // we are in the `POLLING`/`REPOLL` state for the mutex.
        unsafe {
            wake_handle.mutex.start_poll();

            loop {
                let res = future.poll_unpin(&mut cx);
                match res {
                    Poll::Pending => {}
                    Poll::Ready(()) => return wake_handle.mutex.complete(),
                }
                let task = Self {
                    future,
                    wake_handle: wake_handle.clone(),
                    exec,
                };
                match wake_handle.mutex.wait(task) {
                    Ok(()) => return, // we've waited
                    Err(task) => {
                        // someone's notified us
                        future = task.future;
                        exec = task.exec;
                    }
                }
            }
        }
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").field("contents", &"...").finish()
    }
}

impl ArcWake for WakeHandle {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if let Ok(task) = arc_self.mutex.notify() {
            arc_self.exec.state.send(Message::Run(task))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_after_start() {
        {
            let (tx, rx) = mpsc::sync_channel(2);
            let _cpu_pool = ThreadPoolBuilder::new()
                .pool_size(2)
                .after_start(move |_| tx.send(1).unwrap())
                .create()
                .unwrap();

            // After ThreadPoolBuilder is deconstructed, the tx should be dropped
            // so that we can use rx as an iterator.
            let count = rx.into_iter().count();
            assert_eq!(count, 2);
        }
        std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
    }
}
