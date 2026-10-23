use super::super::read_complex_column_links_with_limits;
use super::MultivalueColumn;
use crate::{InspectError, TableSnapshotLimits, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

// Current bounded collection policy, matching Microsoft's documented product
// ceiling. Raw complex links retain their separate whole-table row ceiling.
const MAX_VALUES_PER_PARENT: usize = 100;

/// Resolve and read one ACE 2007/16-family stored multivalue collection column on a
/// stable/offline source. Reject unknown templates, incompatible scalar metadata,
/// ambiguous/orphan references and oversized collections before publication.
/// No lookup query, active content, SQL coercion or attachment decoding executes.
pub fn read_multivalue_column<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueColumn, InspectError> {
    read_multivalue_column_with_limits(
        source,
        scope,
        table,
        column,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

/// Read a complete typed collection under explicit parent/child snapshot limits.
/// Existing schema, reference, per-parent and payload validation still applies.
pub fn read_multivalue_column_with_limits<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.multivalue").entered();
    let links = read_complex_column_links_with_limits(
        source, scope, table, column, limits, budget, cancelled,
    )?;
    read_links(links, budget, cancelled)
}

pub(in crate::table::complex) fn read_links(
    links: crate::ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueColumn, InspectError> {
    let (value_position, _) =
        super::schema::qualified(links.column(), links.child_snapshot().schema())?;
    for parent in links.parents() {
        check(budget, cancelled)?;
        if parent.child_count() > MAX_VALUES_PER_PARENT {
            return Err(InspectError::StructureLimit);
        }
    }
    check(budget, cancelled)?;
    Ok(MultivalueColumn {
        links,
        value_position,
    })
}
