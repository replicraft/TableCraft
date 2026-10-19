use tablecraft_core::limits::LimitError;
use tablecraft_query::QueryError;
use tablecraft_schema::SchemaError;
use thiserror::Error;

/// Sanitized storage errors; no SQLite diagnostic, path, SQL or value payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SqliteError {
    /// SQLite could not open/read/prepare the supplied database.
    #[error("SQLite storage operation failed")]
    Database,
    /// Another connection holds an incompatible lock; no retry is performed.
    #[error("SQLite database is busy")]
    Busy,
    /// The requested object/type is outside the qualified read profile.
    #[error("SQLite object or type is unsupported")]
    Unsupported,
    /// A runtime value disagrees with the declared carrier, or text is not UTF-8.
    #[error("SQLite value does not match the supported carrier")]
    Value,
    /// Configured structural ceiling exceeded.
    #[error("SQLite structural limit exceeded")]
    StructureLimit,
    /// Caller requested cancellation.
    #[error("SQLite read cancelled")]
    Cancelled,
    /// Allocation failed after its budget charge.
    #[error("SQLite adapter allocation failed")]
    Allocation,
    /// Shared budget exhausted.
    #[error(transparent)]
    Limit(#[from] LimitError),
    /// Invalid metadata or row under the shared schema contracts.
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

impl From<rusqlite::Error> for SqliteError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Busy
            }
            Some(rusqlite::ErrorCode::TooBig) => Self::StructureLimit,
            _ => Self::Database,
        }
    }
}

impl From<SqliteError> for QueryError {
    fn from(error: SqliteError) -> Self {
        match error {
            SqliteError::Cancelled => Self::Cancelled,
            SqliteError::Limit(error) => Self::Limit(error),
            SqliteError::Allocation => Self::Allocation,
            SqliteError::StructureLimit => Self::StructureLimit,
            SqliteError::Unsupported => Self::Unsupported,
            SqliteError::Schema(error) => Self::Schema(error),
            _ => Self::Source,
        }
    }
}
