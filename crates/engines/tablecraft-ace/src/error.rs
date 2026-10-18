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
    /// Host cancelled inspection.
    #[error("database inspection cancelled")]
    Cancelled,
    /// The fixed first inspection profile cannot process this input size/count.
    #[error("database inspection structural limit exceeded")]
    StructureLimit,
    /// Fallible allocation failure.
    #[error("database inspection allocation failed")]
    Allocation,
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
