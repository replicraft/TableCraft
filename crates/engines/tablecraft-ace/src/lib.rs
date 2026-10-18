//! Independent, read-only Jet/ACE format inspection. No providers or active content.
#![deny(missing_docs)]
mod catalog;
mod constraints;
mod error;
mod format;
mod header;
mod limits;
mod properties;
mod relationships;
mod table;
mod transaction;
pub use catalog::{CatalogInspection, NativeObjectOrigin, inspect_catalog};
pub use constraints::{
    ConstraintReport, ConstraintViolation, FieldCheck, RelationshipCheck, RelationshipCheckStatus,
    RuleCheckStatus, check_table_constraints,
};
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

pub use relationships::{NativeRelationship, inspect_relationships};
