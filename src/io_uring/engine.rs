use std::collections::VecDeque;

use io_uring::{
    squeue,
    types::{SubmitArgs, Timespec},
    IoUring,
};
use nix::libc;
use slab::Slab;

use super::{cancel::Cancel, Completion, CompletionStatus, Error, Result};

pub struct UringDriver {
    uring: IoUring,
    backlog: VecDeque<squeue::Entry>,
    state: Slab<Box<dyn Completion>>,
    submit_timeout: Timespec,
    min_completions: usize,
}

impl UringDriver {
    pub fn new(entries: u32) -> Result<UringDriver> {
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

    fn clear_backlog(&mut self) -> Result<()> {
        let (submitter, mut sq, _) = self.uring.split();
        loop {
            if sq.is_full() {
                match submitter.submit() {
                    Ok(_) => (),
                    Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => break,
                    Err(err) => return Err(err.into()),
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

    pub fn register(&mut self, mut op: impl Completion + 'static) -> usize {
        let entry = op.as_entry();
        let index = self.state.insert(Box::new(op));
        self.enqueue(entry.user_data(index as _));
        index
    }

    pub fn deregister(&mut self, index: usize) {
        if self.state.try_remove(index).is_none() {
            return;
        }

        let mut op = Cancel::new(index);
        let entry = op.as_entry();
        let index = self.state.insert(Box::new(op));
        self.enqueue(entry.user_data(index as _));
    }

    pub fn run(&mut self) -> Result<()> {
        let args = SubmitArgs::new().timespec(&self.submit_timeout);
        match self
            .uring
            .submitter()
            .submit_with_args(self.min_completions, &args)
        {
            Ok(_) => {}
            Err(e) => match e.raw_os_error() {
                Some(libc::EBUSY) => {}
                Some(libc::ETIME) => {}
                _ => return Err(Error::from(e)),
            },
        }

        self.clear_backlog()?;

        let (_, mut sq, mut cq) = self.uring.split();
        for cqe in &mut cq {
            let user_data = cqe.user_data();

            let state = match self.state.get_mut(user_data as usize) {
                Some(state) => state,
                None => continue,
            };

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
                    let entry = state.as_entry().user_data(user_data as u64);
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
