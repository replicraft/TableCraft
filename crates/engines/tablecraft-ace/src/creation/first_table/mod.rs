//! One explicit schema transition from the immutable canonical blank profile.
mod model;
mod plan;

pub use model::FirstTableDefinition;
pub use plan::{FirstTableError, FirstTablePlan};

#[cfg(test)]
mod tests;
