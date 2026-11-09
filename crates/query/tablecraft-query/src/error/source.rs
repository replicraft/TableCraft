//! Payload-free failures reported by explicitly supplied query sources.
use std::io::ErrorKind;
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Backend-neutral source diagnostics. These categories retain no path, SQL,
/// object name, row value or underlying error message. Cancellation remains the
/// separate `QueryError::Cancelled` contract. A category is diagnostic, never an
/// instruction to retry, fall back to another source or publish a failed prefix.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SourceFailure {
    #[error("I/O failed ({0:?})")]
    Io(ErrorKind),
    #[error("truncated data")]
    Truncated,
    #[error("invalid data")]
    InvalidData,
    #[error("busy")]
    Busy,
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid cursor state")]
    CursorState,
    #[error("constraint failed")]
    Constraint,
    #[error(transparent)]
    Limit(#[from] LimitError),
    #[error("structural limit exceeded")]
    StructureLimit,
    #[error("allocation failed")]
    Allocation,
}
