//! Explicit indexed cached-output reads; physical keys remain ordinary scalars.
use super::{IndexedTableSnapshot, ReadOptions, projected_selection, read_selected};
use crate::{
    IndexReadRequest, InspectError, NativeReadSource, NativeRowReference, TableSnapshot,
    inspect_table_features,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

struct StoredRead<'a> {
    request: IndexReadRequest<'a>,
    table_order: bool,
    references: Option<&'a mut Vec<NativeRowReference>>,
}

fn read<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    options: StoredRead<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let request = options.request;
    if request.maximum_rows == 0 || request.maximum_rows > crate::TableSnapshotLimits::MAX_ROWS {
        return Err(InspectError::InvalidSnapshotLimits);
    }
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: projected_selection(request.selection),
            columns: request.columns,
            maximum_rows: request.maximum_rows,
            legacy_table_limit: false,
            table_order: options.table_order,
            eligible_rows: None,
            row_references: options.references,
            stored_features: Some(&features),
        },
        budget,
        cancelled,
    )
}

/// Read an explicit stored scalar projection in a required ordinary index's order.
/// Requested cached fields require matching ResultType and nonempty Expression
/// declarations; their stored values may be stale. Every physical key remains
/// ordinary and is checked against its row. No formula, conversion, provider or
/// attachment decoder runs; unsupported fields never disappear.
/// Typed selections use the ordinary index profile's pruning and returned-row cap;
/// skipped subtrees/payloads and complete counts remain unchecked for partial reads.
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
    read(
        source,
        scope,
        table,
        index,
        StoredRead {
            request,
            table_order: false,
            references: None,
        },
        budget,
        cancelled,
    )
}

/// Read a stored indexed selection in original logical page/slot order.
/// Matches the ordinary table-order reader's bounded sorting, ownership, pruning
/// and cumulative resource checks, without evaluating cached formulas. This is
/// suitable for automatic index hints whose result order must remain sequential.
pub fn read_stored_table_index_in_table_order<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    request: IndexReadRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read(
        source,
        scope,
        table,
        index,
        StoredRead {
            request,
            table_order: true,
            references: None,
        },
        budget,
        cancelled,
    )
}

/// Read the same explicit stored indexed projection with logical parent identities.
/// One checked original root accompanies each returned row, before forwarding.
/// References are source/schema-local and spend the same cumulative caller budget;
/// they do not certify another source/revision, a complete partial selection or writes.
pub fn read_stored_table_index_with_references<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    request: IndexReadRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexedTableSnapshot, InspectError> {
    let mut references = Vec::new();
    let snapshot = read(
        source,
        scope,
        table,
        index,
        StoredRead {
            request,
            table_order: false,
            references: Some(&mut references),
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
