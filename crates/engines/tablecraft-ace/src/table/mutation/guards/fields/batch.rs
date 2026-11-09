//! Explicit selected-program rich fixed batches; no ordinary save permission.
mod admission;
mod bound;
mod derive;
pub(super) mod frames;
mod indexed;
mod prepared;
pub(in crate::table::mutation::guards) mod projection;
mod requests;
mod targets;

pub(in crate::table::mutation) use admission::SelectedPrograms;
pub(in crate::table::mutation) use bound::SourceFacts;
pub(in crate::table::mutation) use indexed::CheckedTargets;
#[cfg(feature = "journal")]
pub(in crate::table::mutation) use prepared::prepare_indexed;
pub(in crate::table::mutation) use prepared::{Prepared, prepare};
