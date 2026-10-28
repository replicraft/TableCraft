use std::io::ErrorKind;
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free failures shared by inert format readers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReadError {
    /// Host failure, retaining only its portable kind.
    #[error("format I/O failed ({0:?})")]
    Io(ErrorKind),
    /// Required bytes were absent.
    #[error("truncated format structure")]
    Truncated,
    /// Invalid reader counts, seek positions or format geometry.
    #[error("malformed format structure")]
    Malformed,
    /// The caller cancelled the operation.
    #[error("format read cancelled")]
    Cancelled,
    /// Shared resource allowance exhausted.
    #[error(transparent)]
    Limit(#[from] LimitError),
    /// Fallible memory allocation failed.
    #[error("format allocation failed")]
    Allocation,
    /// The qualified format resource envelope was exceeded.
    #[error("format structural limit exceeded")]
    StructureLimit,
    /// An incompatible host file lock prevented I/O.
    #[error("format file lock is busy")]
    LockBusy,
}

impl From<std::io::Error> for ReadError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == ErrorKind::UnexpectedEof {
            Self::Truncated
        } else {
            Self::Io(error.kind())
        }
    }
}

impl From<tablecraft_io::IoError> for ReadError {
    fn from(error: tablecraft_io::IoError) -> Self {
        use tablecraft_io::IoError;
        match error {
            IoError::LockBusy => Self::LockBusy,
            IoError::Io(ErrorKind::UnexpectedEof) => Self::Truncated,
            IoError::Io(kind) => Self::Io(kind),
            IoError::InvalidReadCount
            | IoError::InvalidWriteCount
            | IoError::InvalidSeekPosition => Self::Malformed,
            IoError::Cancelled => Self::Cancelled,
            IoError::Limit(error) => Self::Limit(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::limits::{LimitError, Resource};
    use tablecraft_io::IoError;

    #[test]
    fn neutral_io_conversion_preserves_failure_classes_without_host_messages() {
        for (input, expected) in [
            (
                IoError::Io(ErrorKind::PermissionDenied),
                ReadError::Io(ErrorKind::PermissionDenied),
            ),
            (IoError::Io(ErrorKind::UnexpectedEof), ReadError::Truncated),
            (IoError::InvalidReadCount, ReadError::Malformed),
            (IoError::InvalidWriteCount, ReadError::Malformed),
            (IoError::InvalidSeekPosition, ReadError::Malformed),
            (IoError::Cancelled, ReadError::Cancelled),
            (IoError::LockBusy, ReadError::LockBusy),
        ] {
            assert_eq!(ReadError::from(input), expected);
        }
        let mut budget = tablecraft_core::limits::ResourceBudget::new(0, 0, 0);
        let limit: LimitError = budget.charge(Resource::Bytes, 1).unwrap_err();
        assert_eq!(
            ReadError::from(IoError::Limit(limit)),
            ReadError::Limit(limit)
        );
        let error = std::io::Error::new(ErrorKind::PermissionDenied, "private host details");
        let error = ReadError::from(error);
        assert_eq!(error, ReadError::Io(ErrorKind::PermissionDenied));
        assert!(!error.to_string().contains("private"));
    }
}
