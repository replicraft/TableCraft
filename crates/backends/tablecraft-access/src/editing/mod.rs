//! Native editing adapters and explicit SQL planning services.

mod editor;
mod error;
#[cfg(feature = "journal")]
pub mod journal;
pub(crate) mod snapshot;
pub mod sql;

pub use editor::NativeEditor;
pub use error::NativeEditError;
