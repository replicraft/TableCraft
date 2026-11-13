//! Native scalar-table, bounded edit and inert saved-query syntax adapter. The
//! engine and query library do not depend on this adapter. Hosts provide a
//! stable/offline source and work budget.
#![deny(missing_docs)]
mod collation;
/// Fresh canonical ACE 16 construction with explicit durable publication.
#[cfg(feature = "ace16-create")]
pub mod creation;
pub use collation::{FrenchLegacyCollation, GeneralCollation, NativeCollationResolver};
mod input;
pub use input::{AccessInput, AccessInputError, inspect_input_envelope};
mod browsing;
pub use browsing::{
    BrowsePage, BrowserCursor, NativeBrowser, ObjectEntry, ObjectInventory, TableDescription,
};
mod source;
pub use source::{
    CalculationError, CalculationView, ExternalNativeSource, ExternalSourceError, LinkSourceError,
    LinkedNativeSource, NativeExternalMapping, NativeLinkMapping, NativeSavedReadError,
    NativeSavedSelectError, NativeSource, NativeSourceHandle, NativeTableIdentity,
    PreparedCalculation, RecomputedNativeSource,
};
pub mod editing;
/// Explicit disposable engineering-copy preparation, separate from ordinary native saves.
#[cfg(feature = "ace16-write-preview")]
pub mod engineering;
pub use editing::{NativeEditError, NativeEditor};
mod saved;
#[cfg(feature = "journal")]
pub use saved::{SavedQuerySqlEditError, prepare_saved_query_sql_expression_edits};
pub use saved::{
    SavedRead, SavedReadError, SavedSelect, SavedSelectError, reconstruct_saved_read,
    reconstruct_saved_select,
};

mod template;
pub use template::TemplateSource;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use saved::{SavedQueryCreationError, SavedQueryCreationOptions, prepare_saved_query_creation};
