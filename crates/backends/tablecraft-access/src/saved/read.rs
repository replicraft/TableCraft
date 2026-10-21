//! Family dispatch for inert native retrieval-query reconstruction.
use super::{SavedRead, SavedReadError, compile, context::Context, fragments, union};
use tablecraft_ace::SavedQueryDefinition;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::ParseLimits;

/// Reconstruct qualified stored SELECT/UNION metadata as owned read syntax.
/// This never binds, resolves sources, executes calls or grants owner authority.
/// The SELECT-only entry point retains its original narrower family contract.
pub fn reconstruct_saved_read(
    definition: SavedQueryDefinition,
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SavedRead, SavedReadError> {
    if cancelled() {
        return Err(SavedReadError::Cancelled);
    }
    match definition.origin().flags() & fragments::QUERY_FAMILY_MASK {
        fragments::SELECT_FAMILY => {
            compile::reconstruct_saved_select(definition, limits, budget, cancelled)
        }
        union::FAMILY => {
            let _span = tracing::debug_span!("access.query.reconstruct").entered();
            let mut context = Context::new(budget, cancelled, limits);
            union::reconstruct(definition, &mut context)
        }
        _ => Err(SavedReadError::Unsupported),
    }
}
