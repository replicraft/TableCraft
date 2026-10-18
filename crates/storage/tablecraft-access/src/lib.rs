//! Read-only native scalar-table adapter. The engine and query library do not
//! depend on this adapter. Hosts provide a stable/offline source and work budget.
#![deny(missing_docs)]
mod source;
pub use source::NativeSource;
