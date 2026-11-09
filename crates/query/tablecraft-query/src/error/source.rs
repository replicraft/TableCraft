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
    /// An operating-system I/O category, without its private error message.
    #[error("I/O failed ({0:?})")]
    Io(ErrorKind),
    /// Required source bytes or records ended prematurely.
    #[error("truncated data")]
    Truncated,
    /// Stored source data failed structural or content validation.
    #[error("invalid data")]
    InvalidData,
    /// The explicitly selected source cannot currently accept this operation.
    #[error("busy")]
    Busy,
    /// The source rejected the supplied request shape or bounds.
    #[error("invalid request")]
    InvalidRequest,
    /// A cursor cannot perform the requested operation in its current state.
    #[error("invalid cursor state")]
    CursorState,
    /// A source-owned validation or constraint failed.
    #[error("constraint failed")]
    Constraint,
    /// The source exhausted its own cumulative resource allowance.
    #[error(transparent)]
    Limit(#[from] LimitError),
    /// A source structural ceiling, independent of query work, was exceeded.
    #[error("structural limit exceeded")]
    StructureLimit,
    /// The source could not allocate checked storage.
    #[error("allocation failed")]
    Allocation,
}
