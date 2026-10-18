use crate::IoError;
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Sanitized protocol errors, without file paths or image contents.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum JournalError {
    /// Underlying bounded I/O, synchronization or cancellation failure.
    #[error(transparent)]
    Io(#[from] IoError),
    /// Invalid/truncated framing, digest, marker or unsupported journal version.
    #[error("invalid image journal")]
    InvalidJournal,
    /// Target length/content does not match the expected transaction images.
    #[error("journal target does not match")]
    TargetMismatch,
    /// A journal cannot be overwritten/reused implicitly.
    #[error("journal storage is not empty")]
    JournalNotEmpty,
    /// Only equal, nonempty image lengths up to the protocol limit are supported.
    #[error("unsupported image transaction size")]
    ImageSize,
    /// A bounded fallible allocation failed.
    #[error("image journal allocation failed")]
    Allocation,
    /// Shared cumulative allocation/read/write/work allowance exhausted.
    #[error(transparent)]
    Limit(#[from] LimitError),
}

/// Commit may have changed the target or persisted its marker. Never infer
/// rollback from an error; inspect/recover the retained journal before proceeding.
#[derive(Debug, Error)]
#[error("image commit failed; explicit recovery required: {0}")]
pub struct CommitError(#[source] pub(super) JournalError);
