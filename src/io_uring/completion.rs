use io_uring::{cqueue, squeue};

pub enum CompletionStatus {
    Armed,
    Rearm,
    Finalized,
}

pub trait Completion: Send {
    fn resolve(&self, value: cqueue::Entry) -> CompletionStatus;
    fn as_entry(&mut self) -> squeue::Entry;
}
