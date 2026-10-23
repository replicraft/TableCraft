use std::io::ErrorKind;
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// I/O failures omit host error messages, paths and payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IoError {
    /// A cooperating handle holds an incompatible OS file lock.
    #[error("file lock is busy")]
    LockBusy,
    /// Host I/O/synchronization failure or EOF, retaining only its portable kind.
    #[error("I/O failed ({0:?})")]
    Io(ErrorKind),
    /// A reader reported more bytes than the supplied buffer can hold.
    #[error("reader returned an invalid byte count")]
    InvalidReadCount,
    /// A writer reported consuming more bytes than supplied.
    #[error("writer returned an invalid byte count")]
    InvalidWriteCount,
    /// An absolute seek reported a position different from the requested offset.
    #[error("seek returned an unexpected position")]
    InvalidSeekPosition,
    /// The host cancelled the operation.
    #[error("I/O cancelled")]
    Cancelled,
    /// The shared work allowance was exhausted.
    #[error(transparent)]
    Limit(#[from] LimitError),
}
impl From<std::io::Error> for IoError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind())
    }
}
