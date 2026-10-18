use std::io::ErrorKind;
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// I/O failures omit host error messages, paths and payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IoError {
    /// Host read/seek failure or EOF, retaining only its portable kind.
    #[error("I/O failed ({0:?})")]
    Io(ErrorKind),
    /// A reader reported more bytes than the supplied buffer can hold.
    #[error("reader returned an invalid byte count")]
    InvalidReadCount,
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
