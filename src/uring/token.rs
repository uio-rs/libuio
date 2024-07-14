use std::{os::fd::OwnedFd, sync::mpsc, task::Waker};

use super::{AsyncResult, Result};

/// Represents a events metadata that the [super::Uring] uses to track state of a event through
/// it's lifecycle.
pub(crate) enum Token {
    Cancel,
    Incoming {
        sender: mpsc::Sender<Result<OwnedFd>>,
        waker: Option<Waker>,
    },
    Accept {
        result: AsyncResult<Result<OwnedFd>>,
        waker: Option<Waker>,
    },
    Recv {
        result: AsyncResult<Result<(usize, bool)>>,
        waker: Option<Waker>,
    },
    Send {
        result: AsyncResult<Result<usize>>,
        waker: Option<Waker>,
    },
    Connect {
        result: AsyncResult<Result<()>>,
        waker: Option<Waker>,
    },
}

impl Token {
    /// Overide this token's waker instance so that when we eventually do get the completion event
    /// we end up waking the correct task.
    pub(crate) fn set_waker(&mut self, new: Waker) {
        use Token::*;
        match self {
            // This is truely unreachable, as this token never has a waker in the first place it
            // can't be called like this. As the only possible way to have a set_waker call is from
            // the net packet implementation which this token can't be used in.
            Cancel => unreachable!(),
            Accept { ref mut waker, .. } => waker.replace(new),
            Incoming { ref mut waker, .. } => waker.replace(new),
            Recv { ref mut waker, .. } => waker.replace(new),
            Send { ref mut waker, .. } => waker.replace(new),
            Connect { ref mut waker, .. } => waker.replace(new),
        };
    }
}
