//! Explicit native fragment adaptation, separate from storage and execution.
mod compile;
mod context;
mod error;
mod fragments;
mod join_conditions;
mod parameters;
mod predicate;
mod relations;

pub use compile::reconstruct_saved_select;
pub use error::SavedSelectError;
use tablecraft_ace::SavedQueryDefinition;
use tablecraft_sql::query::Query;

/// Reconstructed syntax and its original native fragments. This is not a bound
/// query or permission to execute it; sources, functions and parameters remain
/// unresolved. In particular, owner-access syntax grants no authority.
pub struct SavedSelect {
    definition: SavedQueryDefinition,
    query: Query,
}
impl SavedSelect {
    /// Original scoped identity, origin flags and unmodified stored fragments.
    pub fn definition(&self) -> &SavedQueryDefinition {
        &self.definition
    }
    /// TableCraft-owned parameter declarations and SELECT statement, without
    /// upstream parser types, prompting or value binding.
    pub fn query(&self) -> &Query {
        &self.query
    }
}
