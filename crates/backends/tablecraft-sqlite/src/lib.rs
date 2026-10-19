//! Read-only SQLite snapshots feeding TableCraft's query evaluator.
//! No caller SQL is forwarded to SQLite; views, virtual tables and generated
//! columns are outside this initial profile. No async or GUI host is required.
#![deny(missing_docs)]

mod error;
mod limits;
mod source;
mod work;

#[cfg(feature = "async")]
mod worker;

pub use error::SqliteError;
pub use source::SqliteSource;

#[cfg(feature = "async")]
pub use worker::{AsyncError, AsyncOptions, AsyncReader};
