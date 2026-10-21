use tablecraft_core::limits::LimitError;
use tablecraft_sql::ParseError;
use thiserror::Error;

/// Sanitized reconstruction failures, without native names or SQL payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SavedReadError {
    /// Query family, native fragment or identifier outside the qualified profile.
    #[error("saved query metadata is unsupported")]
    Unsupported,
    /// Conflicting, missing or ambiguous metadata in the supported profile.
    #[error("saved query metadata is inconsistent")]
    InvalidMetadata,
    /// Stored expression syntax rejected by the bounded Access parser.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// Cumulative operation budget rejected before the next work/allocation.
    #[error(transparent)]
    Limit(#[from] LimitError),
    /// Cooperative cancellation, including before publication.
    #[error("saved query reconstruction cancelled")]
    Cancelled,
    /// A fallible host allocation failed.
    #[error("saved query allocation failed")]
    Allocation,
    /// Fragment expansion exceeds the caller's clamped parser byte limit.
    #[error("saved query syntax limit exceeded")]
    SyntaxLimit,
}
