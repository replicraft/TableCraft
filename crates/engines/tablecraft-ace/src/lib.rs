//! Independent, read-only Jet/ACE format inspection. No providers or active content.
#![deny(missing_docs)]
mod error;
mod header;
pub use error::InspectError;
pub use header::{FileFormat, Header, inspect_header};
