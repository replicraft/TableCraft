//! Native editing adapters and explicit SQL planning services.

mod editor;
mod error;
pub(crate) mod snapshot;
pub mod sql;

pub use editor::NativeEditor;
pub use error::NativeEditError;
