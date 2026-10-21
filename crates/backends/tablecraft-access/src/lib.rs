//! Native scalar-table, bounded edit and inert saved-query syntax adapter. The
//! engine and query library do not depend on this adapter. Hosts provide a
//! stable/offline source and work budget.
#![deny(missing_docs)]
mod collation;
pub use collation::{GeneralCollation, NativeCollationResolver};
mod input;
pub use input::{AccessInput, AccessInputError};
mod browsing;
pub use browsing::{
    AttachmentColumn, BrowserCursor, MultivaluePageCell, MultivaluePageValue, MultivalueTablePage,
    NativeBrowser, NativeTableStorage, StoredAttachment, StoredCollectionColumn,
    StoredCollectionItem, StoredCollectionPageCell, StoredCollectionPageValue,
    StoredCollectionTablePage, StoredTablePage, StoredVersion, TableBatch, TableDescription,
    TableEntry, TableInventory, TablePage, TablePageRequest, VersionHistoryColumn,
};
mod source;
pub use source::{NativeSavedReadError, NativeSavedSelectError, NativeSource};
mod editing;
pub use editing::{NativeEditError, NativeEditor};
mod saved;
pub use saved::{
    SavedRead, SavedReadError, SavedSelect, SavedSelectError, reconstruct_saved_read,
    reconstruct_saved_select,
};
