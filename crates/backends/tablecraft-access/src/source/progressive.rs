//! Explicit cursor-backed scalar projections without snapshot cache publication.
use super::{NativeSource, query_error};
use tablecraft_ace::{NativeReadSource, NativeTableStorage, TableCursor};
use tablecraft_db::{ProjectedRow, validate_projection};
use tablecraft_query::QueryError;

impl<R: NativeReadSource> NativeSource<'_, R> {
    pub(super) fn progressive_scan(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let _span = tracing::debug_span!("native.source.progressive").entered();
        let entry = self.entry(table)?;
        if entry.origin.storage != NativeTableStorage::Local
            || entry.index.is_some()
            || self.multivalues
            || self.collections
            || self.decoded_attachments
        {
            return Err(QueryError::Unsupported);
        }

        let columns = self.schema_columns(table)?;
        validate_projection(indices, columns.len())?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
        let mut source = self
            .source
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        let names = declared
            .selected_names(indices, &mut budget)
            .map_err(query_error)?;
        let open = if self.stored_calculations && declared.has_cached(Some(indices)) {
            TableCursor::open_stored_projection
        } else {
            TableCursor::open_projection
        };
        let mut cursor = open(
            &mut *source,
            self.scope,
            &entry.name,
            &names,
            &mut budget,
            self.cancelled,
        )
        .map_err(query_error)?;
        if !declared.matches_native(cursor.schema())
            || cursor.columns().len() != indices.len()
            || !cursor
                .columns()
                .iter()
                .zip(indices)
                .all(|(column, index)| declared.columns.get(*index) == Some(column))
        {
            return Err(QueryError::SchemaChanged);
        }
        drop(budget);

        // Snapshot retention ceilings do not cap this cursor. Native allowances
        // remain cumulative across all fetches, including work before a failure.
        // Existing complete snapshots are deliberately neither read nor filled.
        let mut delivered = 0_u32;
        loop {
            let batch = {
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                cursor
                    .next_page(
                        self.snapshot_limits.batch_rows(),
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?
            };
            if batch.offset() != delivered {
                return Err(QueryError::Source);
            }
            for row in batch.rows() {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                if row.len() != indices.len() {
                    return Err(QueryError::SchemaChanged);
                }
                visitor(ProjectedRow::dense(row))?;
                delivered = delivered.checked_add(1).ok_or(QueryError::StructureLimit)?;
            }
            if batch.reached_end() {
                if delivered != cursor.schema().declared_rows() || delivered != cursor.position() {
                    return Err(QueryError::Source);
                }
                return if (self.cancelled)() {
                    Err(QueryError::Cancelled)
                } else {
                    Ok(())
                };
            }
        }
    }
}
