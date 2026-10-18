//! Independent, read-only Jet/ACE format inspection. No providers or active content.
#![deny(missing_docs)]
mod catalog;
mod error;
mod format;
mod header;
mod table;
pub use catalog::{CatalogInspection, NativeObjectOrigin, inspect_catalog};
pub use error::InspectError;
pub use header::{FileFormat, Header, inspect_header};
pub use table::{NativeColumn, TableSchema, inspect_table_schema};
