//! Explicit native fragment adaptation, separate from storage and execution.
mod compile;
mod context;
mod crosstab;
mod error;
mod fragments;
mod identifier;
mod parameters;
mod predicate;
mod read;
mod relations;
mod union;

pub use compile::reconstruct_saved_select;
pub use error::SavedReadError;
/// Compatibility name for SELECT reconstruction errors.
pub use error::SavedReadError as SavedSelectError;
pub use read::reconstruct_saved_read;
use tablecraft_ace::SavedQueryDefinition;
use tablecraft_sql::query::Query;

/// Reconstructed syntax and its original native fragments. This is not a bound
/// query or permission to execute it; sources, functions and parameters remain
/// unresolved. In particular, owner-access syntax grants no authority.
pub struct SavedRead {
    definition: SavedQueryDefinition,
    query: Query,
}
/// Compatibility name for the SELECT-only reconstruction result.
pub type SavedSelect = SavedRead;
impl SavedRead {
    /// Original scoped identity, origin flags and unmodified stored fragments.
    pub fn definition(&self) -> &SavedQueryDefinition {
        &self.definition
    }
    /// TableCraft-owned parameter declarations and read statement, without
    /// upstream parser types, prompting or value binding.
    pub fn query(&self) -> &Query {
        &self.query
    }
}
