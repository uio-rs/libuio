use io_uring::{opcode, types::CancelBuilder};

use super::{Completion, CompletionStatus};

/// A cancel event operation, that will target a given state index. This is a best effort operation
/// which will attempt to cancel any and all operations associated with the given index. Generally
/// this is used during a drop of a given Future or Completion event inegrated with the uring loop.
pub struct Cancel {
    index: usize,
}

impl Cancel {
    /// Create a new [Cancel] event targeting the given state index.
    pub fn new(index: usize) -> Cancel {
        Cancel { index }
    }
}

impl Completion for Cancel {
    fn resolve(&self, _: io_uring::cqueue::Entry) -> CompletionStatus {
        CompletionStatus::Finalized
    }

    fn as_entry(&mut self) -> io_uring::squeue::Entry {
        let cancel = CancelBuilder::user_data(self.index as u64).all();
        opcode::AsyncCancel2::new(cancel).build()
    }
}
