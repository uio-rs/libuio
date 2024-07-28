use std::{
    sync::Arc,
    task::{Context, Poll},
};

use futures::{
    pin_mut,
    task::{waker, ArcWake},
    Future,
};

use crate::context;

struct DummyWaker;

impl ArcWake for DummyWaker {
    fn wake_by_ref(_arc_self: &Arc<Self>) {
        // Do nothing
    }
}

/// Run the supplied future to completion blocking the current thread until the future is complete.
/// This is generally used in `main()` fn's such that the application waits for the main async task
/// to complete before exiting. This method also takes care of driving any and all I/O events that
/// are registered during execution of the future. Note that this is not meant for non-I/O heavy
/// futures. Any computation heavy workloads should be [crate::executor::spawn]'ed from the main
/// async routine to offload their work to the thread pool.
///
/// # Examples
///
/// ```no_run
/// use libuio::executor;
///
/// #[libuio::main]
/// async fn main() -> Result<(), String> {
///     executor::block_on(async {
///         // Do some async work!
///     });
///     // We won't get here until the above async closure completes.
///     Ok(())
/// }
/// ```
///
/// # Panics
///
/// This method may panic if an unrecoverable I/O error occurs.
pub fn block_on<F: Future>(f: F) -> F::Output {
    pin_mut!(f);
    let waker = waker(Arc::new(DummyWaker));
    let mut cx = Context::from_waker(&waker);
    loop {
        // Grab our thread local io_uring and run it.
        context::uring().run().expect("Failed to run I/O loop.");
        if let Poll::Ready(result) = f.as_mut().poll(&mut cx) {
            return result;
        }
    }
}
