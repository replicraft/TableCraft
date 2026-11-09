pub(crate) mod database;
mod decode;
#[cfg(feature = "journal")]
pub(crate) mod framing;
mod inspect;
mod metadata;

pub use database::{
    DatabaseNameAutoCorrect, DatabaseProperties, DatabaseQueryMode, inspect_database_properties,
};
pub use inspect::inspect_table_properties;
pub use metadata::{NativeProperty, PropertyGroup, PropertyGroupKind, TableProperties};
