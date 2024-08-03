use std::{collections::VecDeque, io};

use io_uring::{
    opcode,
    squeue::{self, Flags},
    types::{CancelBuilder, SubmitArgs, Timespec},
    IoUring,
};
use nix::libc;
use slab::Slab;

use super::{Completion, CompletionStatus};

/// A IO Uring driver for registering and monitoring I/O events and integration in a low level
/// aasync framework. This leverages an internal [io_uring::IoUring] to monitor and handle I/O
/// events using the linux io_uring framework. This driver is meant to run 1:1 with the number of
/// executor threads, and handles any/all I/O bound work for the executor.
///
/// The general flow is that using the [crate::context::uring] static method to pull out a
/// [thread_local::ThreadLocal] copy of the [UringDriver] all [crate::net] implementations can
/// register theiry I/O events and the executor can drive the I/O by calling [UringDriver::run] on
/// each iteration of the event loop. The [UringDriver::run] call internall uses [io_uring::Submitter::submit_with_args]
/// using a predefined [Timespec] and minimum number of completions.
///
/// I/O bound implementaions must call [UringDriver::register] with a type that implements the [Completion]
/// trait. This [Completion] is used to both generate the internal [io_uring::opcode] that is used
/// to register the event with the underlying io_uring, but also allos for passing back the result
/// of that event once its complete.
pub struct UringDriver {
    uring: IoUring,
    backlog: VecDeque<squeue::Entry>,
    state: Slab<Box<dyn Completion>>,
    submit_timeout: Timespec,
    min_completions: usize,
}

impl UringDriver {
    /// Create a new [UringDriver] with the specified maximum number of events in flight.
    ///
    /// # Errors
    ///
    /// This method will error if the kernel doesn't support the io_uring features we need, or is
    /// otherwise unable to create the necessary kernel and userspace abstractions to use the ring.
    pub fn new(entries: u32) -> io::Result<UringDriver> {
        let uring = IoUring::builder()
            // .setup_defer_taskrun()
            // .setup_single_issuer()
            .build(entries)?;

        let backlog = VecDeque::with_capacity(1024);
        let state = Slab::with_capacity(1024);
        let submit_timeout = Timespec::new().nsec(100_000_000);
        let min_completions = 1;

        Ok(UringDriver {
            uring,
            backlog,
            state,
            submit_timeout,
            min_completions,
        })
    }

    fn clear_backlog(&mut self) -> io::Result<()> {
        let (submitter, mut sq, _) = self.uring.split();
        loop {
            if sq.is_full() {
                match submitter.submit() {
                    Ok(_) => (),
                    Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => break,
                    Err(err) => return Err(err),
                }
            }
            sq.sync();

            match self.backlog.pop_front() {
                Some(sqe) => unsafe {
                    let _ = sq.push(&sqe);
                },
                None => break,
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, entry: squeue::Entry) {
        // Push the new entry onto the submission queue, or fallback to our local VecDeque on
        // error. The error in question here, is a queue full error, and is meant to be retried,
        // which is handled in the clear_backlog() fn above.
        unsafe {
            if self.uring.submission().push(&entry).is_err() {
                self.backlog.push_back(entry);
            }
        }
    }

    /// Register a new event on the io_uring, this will handle storing the passed in [Completion]
    /// and registering it with the io_uring. Once done it will return the state index to be used
    /// for calls to [UringDriver::deregister] in the event the future that generates this
    /// [Completion] is dropped before it completes.
    pub fn register(&mut self, mut op: impl Completion + 'static) -> usize {
        let entry = op.as_entry();
        let index = self.state.insert(Box::new(op));
        self.enqueue(entry.user_data(index as _));
        index
    }

    /// Remove an event from the io_uring, this is a best effort attempt at deregistering a given
    /// event. It will remove the state object, and then issue an async cancel event to cleanup
    /// pending events if they still happen to be on the io_uring. Note this will not guarantee
    /// that the event doesn't trigger before the canel finishes.
    pub fn deregister(&mut self, index: usize) {
        if self.state.try_remove(index).is_none() {
            return;
        }

        let cancel = CancelBuilder::user_data(index as u64).all();
        let entry = opcode::AsyncCancel2::new(cancel)
            .build()
            .flags(Flags::SKIP_SUCCESS); // Nothing to do on success.
        self.enqueue(entry);
    }

    /// Execute an iteration of the io_uring event loop, this will handle submitting any pending
    /// events in the submission queue, and then wait for the configured number of completions or
    /// the timeout expires. It will than handle any completed events and their results before
    /// returning control back to the caller which should then check for any now awoken async tasks
    /// with pending data to hand off.
    pub fn run(&mut self) -> io::Result<()> {
        // First we need to create new [SubmitArgs] such that we can supply our timeout, since we
        // do not want to block the overall event loop in the executor for an indeterminate period
        // of time potentially starving tasks from execution time.
        let args = SubmitArgs::new().timespec(&self.submit_timeout);

        // Now we submit any pending events in our submission queue and we wait.
        match self
            .uring
            .submitter()
            .submit_with_args(self.min_completions, &args)
        {
            Ok(_) => {}
            Err(e) => match e.raw_os_error() {
                Some(libc::EBUSY) => {} // The ring is currently busy just continue on.
                Some(libc::ETIME) => {} // We timed out just continue on.
                _ => return Err(e),     // The ring is broken terminate.
            },
        }

        // Now we clear any submission events we can that overflowed into our backlog, this should
        // usually be empty hopefully, but just in case lets make sure to handle them.
        self.clear_backlog()?;

        // Finally iterate over any completion events we have, looking up their state objects and
        // calling [Completion::resolve] on any completed events.
        let (_, mut sq, mut cq) = self.uring.split();
        for cqe in &mut cq {
            let user_data = cqe.user_data();

            // Lookup the state for this event, and if not found just drop the completion and
            // continue onto the next one.
            let state = match self.state.get_mut(user_data as usize) {
                Some(state) => state,
                None => continue,
            };

            // Resolve the [Completion] and handle the result.
            use CompletionStatus::*;
            match state.resolve(cqe) {
                Armed => {
                    // Do nothing we are already armed, and we don't want to remove the state yet
                    // since this is likely a multi-shot event and we are awaiting new events to be
                    // generated.
                }
                Rearm => {
                    // We have a multi-shot requesting that we re-arm it, so lets go ahead and do
                    // that so that we continue to get new updates.
                    let entry = state.as_entry().user_data(user_data);
                    unsafe {
                        if sq.push(&entry).is_err() {
                            self.backlog.push_back(entry);
                        }
                    }
                }
                Finalized => {
                    // Our event is handled and done, go ahead and clean up our state entry so its
                    // slot can be reused.
                    self.state.remove(user_data as usize);
                }
            };
        }
        Ok(())
    }
}
