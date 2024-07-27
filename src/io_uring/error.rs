use core::result;
use thiserror::Error;

/// A helper type for wrapping a [result::Result] such that we can reduce noise in our signatures.
pub type Result<T> = result::Result<T, Error>;

/// An error representing a failure interacting with the underlying io_uring, or related errors.
#[derive(Debug, Error)]
pub enum Error {
    #[error("io_uring I/O operation failed: {0}")]
    IO(
        #[source]
        #[from]
        std::io::Error,
    ),
}
