use crate::QueryError;
use thiserror::Error;

/// Typed query/backend failures without SQL, identifiers or row contents.
#[derive(Debug, Error)]
pub enum UpdateError<E: std::error::Error> {
    /// Binding, scalar evaluation, allocation, cancellation or work failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Backend rejected the complete candidate; no durable query writes occur.
    #[error("update backend rejected the candidate: {0}")]
    Backend(#[source] E),
}
