use core::result;
use io_uring::squeue;
use std::{io, sync::mpsc::SendError};
use thiserror::Error;

/// A helper type for wrapping a [result::Result] such that we can reduce noise in our signatures.
pub type Result<T> = result::Result<T, Error>;

/// An error representing a failure interacting with the underlying io_uring, or related errors.
#[derive(Debug, Error)]
pub enum Error {
    #[error("encountered unexpected IO error: {0}")]
    IO(
        #[from]
        #[source]
        io::Error,
    ),
    #[error("failed to submit new entry to submission queue: {0}")]
    Push(
        #[from]
        #[source]
        squeue::PushError,
    ),
    #[error("failed to send response to future consumer: {0}")]
    SendError(String),
    #[error("client disconnected unexpectedly")]
    Disconnected,
}

impl<T> From<SendError<T>> for Error {
    fn from(value: SendError<T>) -> Self {
        Self::SendError(value.to_string())
    }
}

impl From<i32> for Error {
    fn from(value: i32) -> Self {
        let err = io::Error::from_raw_os_error(value);
        Self::IO(err)
    }
}
