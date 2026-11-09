//! One explicit schema transition from the immutable canonical blank profile.
mod model;
mod plan;
mod source;

pub use model::FirstTableDefinition;
pub use plan::{FirstTableError, FirstTablePlan};
pub use source::first_table_source_available;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod manual_tests;
