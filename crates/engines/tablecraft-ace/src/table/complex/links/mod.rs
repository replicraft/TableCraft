mod error;
mod metadata;
mod read;
pub use error::ComplexReferenceViolation;
pub use metadata::{ComplexColumnLinks, ComplexParentLink};
pub use read::read_complex_column_links;
pub(in crate::table::complex) use read::read_resolved;
