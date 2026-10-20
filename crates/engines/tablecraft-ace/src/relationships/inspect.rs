use super::{NativeRelationship, decode};
use crate::{InspectError, table::read_selected};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Inspect bounded native relationship declarations from MSysRelationships.
/// Grouping validates complete unique ordinals and consistent endpoint/flag/count
/// metadata. Referenced tables/fields can remain unresolved; no provider opens.
/// Unknown flags remain raw with unqualified rule semantics, never disabled rules.
pub fn inspect_relationships<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<NativeRelationship>, InspectError> {
    let _span = tracing::debug_span!("native.relationships.read").entered();
    let table = read_selected(
        source,
        scope,
        "MSysRelationships",
        Some(&decode::COLUMNS),
        budget,
        cancelled,
    )?;
    decode::decode(table.columns(), table.rows(), budget, cancelled)
}
