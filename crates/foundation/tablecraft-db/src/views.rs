//! Borrowed storage-neutral row and scalar collection views.
mod collection;
mod projection;

pub use collection::MultivalueList;
pub use projection::{ProjectedRow, ProjectionError, validate_projection};
