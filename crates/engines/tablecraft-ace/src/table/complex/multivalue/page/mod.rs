//! Explicit mixed scalar/collection pages, without flattening or lookup execution.
mod metadata;
mod read;
pub use metadata::{MultivaluePageCell, MultivaluePageValue, MultivalueTablePage};
pub use read::read_multivalue_table_page;
