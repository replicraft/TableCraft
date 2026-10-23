mod error;
mod metadata;
mod read;
pub use error::ComplexReferenceViolation;
pub use metadata::{ComplexColumnLinks, ComplexParentLink};
pub(in crate::table::complex) use read::read_resolved;
pub use read::{read_complex_column_links, read_complex_column_links_with_limits};
