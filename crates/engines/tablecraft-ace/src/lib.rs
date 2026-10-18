//! Independent, read-only Jet/ACE format inspection. No providers or active content.
#![deny(missing_docs)]
mod catalog;
mod error;
mod format;
mod header;
mod limits;
mod properties;
mod table;
mod transaction;
pub use catalog::{CatalogInspection, NativeObjectOrigin, inspect_catalog};
pub use error::InspectError;
pub use format::profiles::ReadProfile;
pub use header::{FileFormat, Header, inspect_header};
pub use table::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
pub use table::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};
pub use table::{IndexConstraintViolation, IndexConstraints};
pub use table::{Int32IndexSelection, read_table_index, read_table_index_range};
pub use table::{NativeColumn, TableSchema, TableSnapshot, inspect_table_schema, read_table};
pub use transaction::OfflineReadTransaction;

pub use properties::{
    NativeProperty, PropertyGroup, PropertyGroupKind, TableProperties, inspect_table_properties,
};
