use thiserror::Error;

/// Inspection errors omit paths, names, keys and database contents.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InspectError {
    /// Host read/seek failure, without its potentially sensitive message.
    #[error("database I/O failed ({0:?})")]
    Io(std::io::ErrorKind),
    /// Input ended before a required structure.
    #[error("truncated database structure")]
    Truncated,
    /// Invalid signature, pointers, layout or encoding.
    #[error("malformed database structure")]
    Malformed,
    /// Recognized or unknown format outside the implemented profile.
    #[error("database format or layout is not supported")]
    Unsupported,
    /// Encoded/encrypted data pages require an unimplemented codec.
    #[error("encoded database pages are not supported")]
    Encoded,
    /// No explicitly named table in the inspected catalog.
    #[error("native table has no catalog binding")]
    UnknownTable,
    /// Host cancelled inspection.
    #[error("database inspection cancelled")]
    Cancelled,
    /// The fixed first inspection profile cannot process this input size/count.
    #[error("database inspection structural limit exceeded")]
    StructureLimit,
    /// Fallible allocation failure.
    #[error("database inspection allocation failed")]
    Allocation,
    /// Invalid shared catalog metadata.
    #[error(transparent)]
    Model(#[from] tablecraft_model::ModelError),
    /// Invalid shared schema identity.
    #[error(transparent)]
    Schema(#[from] tablecraft_schema::SchemaError),
    /// Shared work/read/allocation allowance exhausted.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
}
impl From<std::io::Error> for InspectError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            Self::Truncated
        } else {
            Self::Io(error.kind())
        }
    }
}

impl From<tablecraft_io::IoError> for InspectError {
    fn from(error: tablecraft_io::IoError) -> Self {
        use tablecraft_io::IoError;
        match error {
            IoError::Io(std::io::ErrorKind::UnexpectedEof) => Self::Truncated,
            IoError::Io(kind) => Self::Io(kind),
            IoError::InvalidReadCount => Self::Malformed,
            IoError::Cancelled => Self::Cancelled,
            IoError::Limit(error) => Self::Limit(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use tablecraft_core::limits::{LimitError, Resource};
    use tablecraft_io::IoError;

    #[test]
    fn generic_io_failures_preserve_the_native_error_contract() {
        for (input, expected) in [
            (
                IoError::Io(ErrorKind::UnexpectedEof),
                InspectError::Truncated,
            ),
            (
                IoError::Io(ErrorKind::PermissionDenied),
                InspectError::Io(ErrorKind::PermissionDenied),
            ),
            (IoError::InvalidReadCount, InspectError::Malformed),
            (IoError::Cancelled, InspectError::Cancelled),
            (
                IoError::Limit(LimitError {
                    resource: Resource::Steps,
                    remaining: 0,
                }),
                InspectError::Limit(LimitError {
                    resource: Resource::Steps,
                    remaining: 0,
                }),
            ),
        ] {
            assert_eq!(InspectError::from(input), expected);
        }
    }
}
