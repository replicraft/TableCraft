//! Independent, read-only Jet/ACE format inspection. No providers or active content.
#![deny(missing_docs)]
mod catalog;
mod error;
mod format;
mod header;
mod profile;
mod table;
pub use catalog::{CatalogInspection, NativeObjectOrigin, inspect_catalog};
pub use error::InspectError;
pub use header::{FileFormat, Header, inspect_header};
pub use table::read_table_index;
pub use table::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};
pub use table::{NativeColumn, TableSchema, TableSnapshot, inspect_table_schema, read_table};
