use io_uring::{opcode, types::CancelBuilder};

use super::{Completion, CompletionStatus};

pub struct Cancel {
    index: usize,
}

impl Cancel {
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
