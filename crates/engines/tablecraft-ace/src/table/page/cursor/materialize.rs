//! Explicit bounded snapshot materialization through resumable scalar batches.
use super::*;
use crate::table::snapshot::TableSnapshot;

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
    TableCursor::open(source, scope, name, budget, cancelled)?.into_snapshot(
        TableSnapshotLimits::default(),
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
    TableCursor::open_projection(source, scope, name, selected, budget, cancelled)?.into_snapshot(
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

impl<R: crate::NativeReadSource> TableCursor<'_, R> {
    /// Consume a cursor that has not returned any rows into a complete snapshot.
    /// Opening/fetching and retained storage share caller-owned controls. No partial
    /// snapshot is returned; EOF/count/schema checks precede publication. Index and
    /// collection readers retain their separate ceilings and qualification.
    pub fn into_snapshot(
        mut self,
        limits: TableSnapshotLimits,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableSnapshot, InspectError> {
        let _span = tracing::debug_span!("native.table.materialize_batches").entered();
        if self.failed {
            return Err(InspectError::CursorFailed);
        }
        if self.delivered != 0 {
            return Err(InspectError::CursorAdvanced);
        }
        if self.schema.declared_rows() > limits.maximum_rows() {
            return Err(InspectError::StructureLimit);
        }
        let mut rows = Vec::new();
        loop {
            let batch = self.next_page(limits.batch_rows(), budget, cancelled)?;
            if rows
                .len()
                .checked_add(batch.rows.len())
                .is_none_or(|count| count > limits.maximum_rows() as usize)
            {
                return Err(InspectError::StructureLimit);
            }
            if rows.is_empty() {
                rows = batch.rows;
            } else {
                budget
                    .charge_allocation(batch.rows.len() as u64, size_of::<Vec<Value>>() as u64)?;
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
            columns = self.columns.len(),
            "native batched table completed"
        );
        Ok(TableSnapshot {
            schema: self.schema,
            columns: self.columns,
            rows,
        })
    }
}
