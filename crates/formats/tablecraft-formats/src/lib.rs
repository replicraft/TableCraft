//! Bounded inert package and stored-data codecs, independent of native engines and query execution.
#![deny(missing_docs)]
mod error;
pub use error::ReadError;

/// Qualified single-file CAB framing and bounded payload extraction.
pub mod deployment;
/// Access template ZIP/OPC, XML metadata and stored table-data codecs.
pub mod template;
