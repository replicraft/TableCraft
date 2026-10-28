//! Explicit indexed cached-output reads; physical keys remain ordinary scalars.
use super::{IndexedTableSnapshot, ReadOptions, read_selected};
use crate::{
    IndexProjectionSelection, IndexReadRequest, InspectError, NativeReadSource, TableSnapshot,
    inspect_table_features,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn qualify_request(request: &IndexReadRequest<'_>) -> Result<(), InspectError> {
    if request.maximum_rows == 0 || request.maximum_rows > crate::TableSnapshotLimits::MAX_ROWS {
        return Err(InspectError::InvalidSnapshotLimits);
    }
    if request.selection != IndexProjectionSelection::All {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}

/// Read an explicit stored scalar projection in a required ordinary index's order.
/// Only complete All traversals qualify. Requested cached fields require matching
/// ResultType and nonempty Expression declarations; their stored values may be stale.
/// Every physical key remains ordinary and is checked against its row. No formula,
/// conversion, provider or attachment decoder runs; unsupported fields never disappear.
/// Legacy ordinary indexed APIs retain their separate strict carrier policy.
pub fn read_stored_table_index_with_request<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    request: IndexReadRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    qualify_request(&request)?;
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: None,
            columns: request.columns,
            maximum_rows: request.maximum_rows,
            legacy_table_limit: false,
            table_order: false,
            eligible_rows: None,
            row_references: None,
            stored_features: Some(&features),
        },
        budget,
        cancelled,
    )
}

/// Read the same explicit stored indexed projection with logical parent identities.
/// One checked original root accompanies each returned row, before forwarding.
/// References are source/schema-local and spend the same cumulative caller budget;
/// they do not certify another source/revision or authorize writes.
pub fn read_stored_table_index_with_references<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    request: IndexReadRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexedTableSnapshot, InspectError> {
    qualify_request(&request)?;
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    let mut references = Vec::new();
    let snapshot = read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: None,
            columns: request.columns,
            maximum_rows: request.maximum_rows,
            legacy_table_limit: false,
            table_order: false,
            eligible_rows: None,
            row_references: Some(&mut references),
            stored_features: Some(&features),
        },
        budget,
        cancelled,
    )?;
    if references.len() != snapshot.rows().len() {
        return Err(InspectError::Malformed);
    }
    Ok(IndexedTableSnapshot {
        snapshot,
        references,
    })
}
