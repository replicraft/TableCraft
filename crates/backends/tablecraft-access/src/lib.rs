//! Native scalar-table, bounded edit and inert saved-query syntax adapter. The
//! engine and query library do not depend on this adapter. Hosts provide a
//! stable/offline source and work budget.
#![deny(missing_docs)]
mod collation;
pub use collation::{FrenchLegacyCollation, GeneralCollation, NativeCollationResolver};
mod input;
pub use input::{AccessInput, AccessInputError};
mod browsing;
pub use browsing::{
    AttachmentColumn, BrowsePage, BrowserCursor, CollectionPageRequest, MultivaluePageCell,
    MultivaluePageValue, MultivalueTablePage, NativeBrowser, NativeTableStorage, StoredAttachment,
    StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageCell,
    StoredCollectionPageValue, StoredCollectionTablePage, StoredTablePage, StoredVersion,
    TableBatch, TableDescription, TableEntry, TableInventory, TablePage, TablePageRequest,
    VersionHistoryColumn,
};
mod source;
pub use source::{
    CalculationError, CalculationView, ExternalNativeSource, ExternalSourceError, LinkSourceError,
    LinkedNativeSource, NativeExternalMapping, NativeLinkMapping, NativeSavedReadError,
    NativeSavedSelectError, NativeSource, NativeSourceHandle, NativeTableIdentity,
    PreparedCalculation, RecomputedNativeSource,
};
mod editing;
/// Explicit disposable engineering-copy preparation, separate from ordinary native saves.
#[cfg(feature = "ace16-write-preview")]
pub mod engineering;
pub use editing::{NativeEditError, NativeEditor};
mod saved;
pub use saved::{
    SavedRead, SavedReadError, SavedSelect, SavedSelectError, reconstruct_saved_read,
    reconstruct_saved_select,
};

mod template;
pub use template::TemplateSource;
