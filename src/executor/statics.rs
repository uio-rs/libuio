use std::sync::{Arc, Mutex};

use futures::Future;
use lazy_static::lazy_static;

use super::ThreadPool;

lazy_static! {
    static ref POOL: Arc<Mutex<Option<ThreadPool>>> = Arc::new(Mutex::new(None));
}

pub(crate) fn set_pool(pool: ThreadPool) {
    POOL.lock()
        .expect("failed to lock thread pool: poisoned")
        .replace(pool);
}

/// Spawn a task on the runtime, the future will run on one of the available executor threads and
/// execute concurrently with any other active futures in the runtime. These futures can't return a
/// value due to the nature of their execution.
///
/// # Examples
///
/// ```no_run
/// use libuio::executor;
///
/// #[libuio::main]
/// async fn main() -> Result<(), String> {
///     executor::spawn(async {
///         // Do some async work!
///     });
///     // Do other things here! This will execute immediately after `spawn()` returns, and will
///     // not wait for the async block to be executed.
/// }
///
/// ```
/// # Panics
///
/// This method will panic in the event that the internal locking logic is poisoned, or more likely
/// the runtime hasn't been configured, this can be easily avoided by leveraging the [crate::main]
/// proc macro which will handle configuring and setting up the internal executor.
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    let pool = POOL.lock().expect("failed to lock thread pool: poisoned");
    match pool.as_ref() {
        Some(pool) => pool.spawn_ok(future),
        None => panic!("runtime not configured"),
    };
}
