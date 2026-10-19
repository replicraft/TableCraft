//! Explicit native fragment adaptation, separate from storage and execution.
mod compile;
mod context;
mod error;
mod fragments;
mod predicate;

pub use compile::reconstruct_saved_select;
pub use error::SavedSelectError;
use tablecraft_ace::SavedQueryDefinition;
use tablecraft_sql::select::Select;

/// Reconstructed syntax and its original native fragments. This is not a bound
/// query or permission to execute it; sources, functions and parameters remain
/// unresolved. In particular, owner-access syntax grants no authority.
pub struct SavedSelect {
    definition: SavedQueryDefinition,
    query: Select,
}
impl SavedSelect {
    /// Original scoped identity, origin flags and unmodified stored fragments.
    pub fn definition(&self) -> &SavedQueryDefinition {
        &self.definition
    }
    /// TableCraft-owned SELECT syntax, without upstream parser types.
    pub fn query(&self) -> &Select {
        &self.query
    }
}
