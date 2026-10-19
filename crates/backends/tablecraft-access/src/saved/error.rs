use tablecraft_core::limits::LimitError;
use tablecraft_sql::ParseError;
use thiserror::Error;

/// Sanitized reconstruction failures, without native names or SQL payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SavedSelectError {
    /// Query family, native fragment or identifier outside the qualified profile.
    #[error("saved SELECT metadata is unsupported")]
    Unsupported,
    /// Conflicting, missing or ambiguous metadata in the supported profile.
    #[error("saved SELECT metadata is inconsistent")]
    InvalidMetadata,
    /// Stored expression syntax rejected by the bounded Access parser.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// Cumulative operation budget rejected before the next work/allocation.
    #[error(transparent)]
    Limit(#[from] LimitError),
    /// Cooperative cancellation, including before publication.
    #[error("saved SELECT reconstruction cancelled")]
    Cancelled,
    /// A fallible host allocation failed.
    #[error("saved SELECT allocation failed")]
    Allocation,
    /// Fragment expansion exceeds the caller's clamped parser byte limit.
    #[error("saved SELECT syntax limit exceeded")]
    SyntaxLimit,
}
