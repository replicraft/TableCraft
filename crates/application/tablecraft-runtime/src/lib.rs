//! Shared application sessions and cancellable native read tasks for hosts.
mod controls;
mod display;
mod document;
mod query;
mod rows;
mod source;
mod worker;

pub use worker::Worker;

use std::path::PathBuf;
use zeroize::Zeroize;

/// A typed task failure. Display text contains no credentials or row payloads.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation cancelled")]
    Cancelled,
    #[error("no database is open")]
    NoDatabase,
    #[error("file exceeds the 64 MiB read-session limit")]
    SnapshotLimit,
    #[error("the file changed while it was being opened")]
    SourceChanged,
    #[error("SQL exceeds the 64 KiB command limit")]
    SqlLimit,
    #[error("binary display exceeds the presentation limit")]
    BinaryLimit,
    #[error("unpublished expression bytes cannot be displayed")]
    UnpublishedString,
    #[error("text formatting failed")]
    Formatting,
    #[error(transparent)]
    Input(#[from] tablecraft_access::AccessInputError),
    #[error(transparent)]
    Native(#[from] tablecraft_ace::InspectError),
    #[error(transparent)]
    Io(#[from] tablecraft_io::IoError),
    #[error(transparent)]
    Parse(#[from] tablecraft_sql::ParseError),
    #[error(transparent)]
    Query(#[from] tablecraft_query::QueryError),
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    #[error(transparent)]
    Value(#[from] tablecraft_core::values::ValueError),
    #[error("allocation failed")]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Explicit host requests. Queued and consumed passwords are erased on drop.
pub enum Command {
    Open { path: PathBuf, password: String },
    Page { table: String, offset: u32 },
    Query { sql: String },
}

impl Drop for Command {
    fn drop(&mut self) {
        if let Self::Open { password, .. } = self {
            password.zeroize();
        }
    }
}

pub struct Table {
    pub name: String,
    pub system: bool,
    pub storage: String,
}

pub enum Response {
    Opened {
        title: String,
        format: String,
        tables: Vec<Table>,
    },
    Data {
        table: String,
        offset: u32,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        has_more: bool,
    },
}

#[cfg(test)]
mod tests;
