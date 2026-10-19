//! Native scalar-table, bounded edit and inert saved-query syntax adapter. The
//! engine and query library do not depend on this adapter. Hosts provide a
//! stable/offline source and work budget.
#![deny(missing_docs)]
mod browsing;
pub use browsing::{NativeBrowser, TableInventory, TablePage, TablePageRequest};
mod source;
pub use source::NativeSource;
mod editing;
pub use editing::{NativeEditError, NativeEditor};
mod saved;
pub use saved::{SavedSelect, SavedSelectError, reconstruct_saved_select};
