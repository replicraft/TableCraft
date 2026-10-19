//! Explicit typed collection pages without scalar coercion or active execution.
mod column;
mod metadata;
mod read;
pub use column::{StoredCollectionColumn, StoredCollectionItem};
pub use metadata::{
    StoredCollectionPageCell, StoredCollectionPageValue, StoredCollectionTablePage,
};
pub use read::read_stored_collection_table_page;

pub(in crate::table::complex) use read::{CollectionOptions, read_with_options};
