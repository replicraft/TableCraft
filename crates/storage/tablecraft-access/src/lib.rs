//! Native scalar-table and bounded edit adapter. The engine and query library do not
//! depend on this adapter. Hosts provide a stable/offline source and work budget.
#![deny(missing_docs)]
mod source;
pub use source::NativeSource;
mod editing;
pub use editing::{NativeEditError, NativeEditor};
