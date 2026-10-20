use thiserror::Error;

/// Inspection errors omit paths, names, keys and database contents.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InspectError {
    /// The operating system could not supply secure encryption material.
    #[error("secure encryption randomness is unavailable")]
    EntropyUnavailable,
    /// A rewrite requires a nonempty replacement credential.
    #[error("new database password must not be empty")]
    EmptyPassword,
    /// The native password field cannot represent this credential exactly.
    #[error("database password contains an unsupported character")]
    UnsupportedPassword,
    /// Encryption removal/password change requires an existing protection scheme.
    #[error("database is not encrypted or password-protected")]
    NotEncrypted,
    /// An incompatible OS lock prevents opening an explicit offline transaction.
    #[error("database file lock is busy")]
    LockBusy,
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
    /// An ACE encryption envelope has invalid bounds or a malformed fixed prefix.
    #[error("malformed database encryption envelope")]
    MalformedEncryption,
    /// A recognized envelope requires an unimplemented cipher or parameter profile.
    #[error("database encryption profile is not supported")]
    UnsupportedEncryption,
    /// A qualified password-based codec was opened without explicit credentials.
    #[error("database password is required")]
    PasswordRequired,
    /// The supplied password does not match the native verifier.
    #[error("incorrect database password")]
    IncorrectPassword,
    /// The password exceeds the qualified UTF-16 code-unit bound.
    #[error("database password exceeds the supported length")]
    InvalidPasswordLength,
    /// No explicitly named table in the inspected catalog.
    #[error("native table has no catalog binding")]
    UnknownTable,
    /// No explicitly named native column on the selected table.
    #[error("native column has no binding")]
    UnknownColumn,
    /// A qualified complex storage view has inconsistent scoped references.
    #[error("native complex reference violated: {0}")]
    ComplexReference(#[from] crate::ComplexReferenceViolation),
    /// Explicit identity has no declaration in the captured catalog.
    #[error("native object has no catalog binding")]
    UnknownObject,
    /// No explicitly named native index on the selected table.
    #[error("native index has no binding")]
    UnknownIndex,
    /// Decoded records violate a qualified native index declaration.
    #[error("native index constraint violated: {0}")]
    IndexConstraint(#[from] crate::IndexConstraintViolation),
    /// Explicit native field/relationship audit found a record violation.
    #[error("native record constraint violated: {0}")]
    Constraint(#[from] crate::ConstraintViolation),
    /// Host cancelled inspection.
    #[error("database inspection cancelled")]
    Cancelled,
    /// The fixed first inspection profile cannot process this input size/count.
    #[error("database inspection structural limit exceeded")]
    StructureLimit,
    /// A page request has zero rows or exceeds the per-page ceiling.
    #[error("invalid native table page range")]
    InvalidPageRange,
    /// A failed resumable fetch has made this source-bound cursor terminal.
    #[error("native table cursor cannot continue after a failed fetch")]
    CursorFailed,
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
            IoError::LockBusy => Self::LockBusy,
            IoError::Io(std::io::ErrorKind::UnexpectedEof) => Self::Truncated,
            IoError::Io(kind) => Self::Io(kind),
            IoError::InvalidReadCount => Self::Malformed,
            IoError::InvalidWriteCount => Self::Malformed,
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
            (IoError::LockBusy, InspectError::LockBusy),
            (
                IoError::Io(ErrorKind::UnexpectedEof),
                InspectError::Truncated,
            ),
            (
                IoError::Io(ErrorKind::PermissionDenied),
                InspectError::Io(ErrorKind::PermissionDenied),
            ),
            (IoError::InvalidReadCount, InspectError::Malformed),
            (IoError::InvalidWriteCount, InspectError::Malformed),
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
