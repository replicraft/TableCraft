use crate::SqliteError;
use tablecraft_query::QueryError;
use thiserror::Error;

/// Sanitized asynchronous request/worker failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AsyncError {
    /// This reader already has an admitted job, including abandoned work still
    /// completing cancellation/rollback. The request was not queued.
    #[error("SQLite reader already has an active request")]
    Busy,
    /// The worker closed, failed or could not confirm clean teardown.
    #[error("SQLite worker is unavailable")]
    Worker,
    /// Typed storage rejection without SQLite diagnostics or payloads.
    #[error(transparent)]
    Storage(#[from] SqliteError),
    /// TableCraft binding/evaluation rejection.
    #[error(transparent)]
    Query(#[from] QueryError),
}
