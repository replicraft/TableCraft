//! Explicit bounded snapshot materialization through resumable scalar batches.
use super::*;
use crate::table::snapshot::TableSnapshot;

const SNAPSHOT_BATCH_ROWS: u32 = 256;

/// Materialize a complete bounded scalar snapshot through cursor continuation.
/// Every column must be supported. The snapshot row ceiling remains unchanged;
/// physical reads are bounded per batch and cumulative caller controls never reset.
/// Full usage metadata and every live row/count are checked before publication.
/// Requires a stable/offline source; no native snapshot/locking is acquired.
pub fn read_table_batched<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    collect(
        TableCursor::open(source, scope, name, budget, cancelled)?,
        budget,
        cancelled,
    )
}

/// Materialize an explicit ordinary scalar subset through cursor continuation.
/// Selection follows `TableCursor::open_projection`; unrelated payloads remain
/// unqualified. Complete fetched framing and live counts are checked, including
/// for zero-width rows. No partial snapshot is returned on failure.
pub fn read_table_projection_batched<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    selected: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    collect(
        TableCursor::open_projection(source, scope, name, selected, budget, cancelled)?,
        budget,
        cancelled,
    )
}

fn collect<R: crate::NativeReadSource>(
    mut cursor: TableCursor<'_, R>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let _span = tracing::debug_span!("native.table.materialize_batches").entered();
    if cursor.schema.declared_rows() > TablePageRequest::MAX_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let mut rows = Vec::new();
    loop {
        let batch = cursor.next_page(SNAPSHOT_BATCH_ROWS, budget, cancelled)?;
        if rows.len() + batch.rows.len() > TablePageRequest::MAX_ROWS as usize {
            return Err(InspectError::StructureLimit);
        }
        if rows.is_empty() {
            rows = batch.rows;
        } else {
            budget.charge_allocation(batch.rows.len() as u64, size_of::<Vec<Value>>() as u64)?;
            rows.try_reserve(batch.rows.len())
                .map_err(|_| InspectError::Allocation)?;
            rows.extend(batch.rows);
        }
        if batch.reached_end {
            break;
        }
    }
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    tracing::debug!(
        rows = rows.len(),
        columns = cursor.columns.len(),
        "native batched table completed"
    );
    Ok(TableSnapshot {
        schema: cursor.schema,
        columns: cursor.columns,
        rows,
    })
}
