use thiserror::Error;

/// Snapshot failures without names, row contents or SQL payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotError {
    /// Invalid schema or exact row carrier.
    #[error(transparent)]
    Schema(#[from] tablecraft_schema::SchemaError),
    /// Shared work/resource allowance exhausted.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    /// Host cancelled before completion.
    #[error("database snapshot operation cancelled")]
    Cancelled,
    /// A row belongs to a different snapshot or has no binding.
    #[error("database snapshot row is stale")]
    StaleRow,
    /// Process-local snapshot identities cannot be issued without reuse.
    #[error("database snapshot identity space exhausted")]
    IdentityExhausted,
}
