//! Native editing adapters and explicit SQL planning services.

mod editor;
mod error;
#[cfg(feature = "journal")]
pub mod journal;
#[cfg(all(feature = "offline-write-session", any(unix, windows)))]
pub mod offline;
pub(crate) mod snapshot;
pub mod sql;

pub use editor::NativeEditor;
pub use error::NativeEditError;
