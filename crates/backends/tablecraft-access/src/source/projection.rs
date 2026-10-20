//! Bounded native projected scans and complete projection snapshot reuse.
use super::{
    Entry, NativeSource, query_error, schema::ScalarSchema, selection::projected_selection,
};
use std::io::{Read, Seek};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, TableSnapshot, inspect_table_indexes,
    read_table_index_projection, read_table_projection_batched,
};
use tablecraft_core::limits::Resource;
use tablecraft_query::{
    QueryError, Source,
    source::{ProjectedRow, ScanConstraint, validate_projection},
};

pub(super) struct CachedProjection {
    indices: Vec<usize>,
    snapshot: TableSnapshot,
}
impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn projected_scan(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let columns = self.schema_columns(table)?;
        validate_projection(indices, columns.len())?;
        if indices.len() == columns.len() {
            let mut dense =
                |row: &[tablecraft_core::values::Value]| visitor(ProjectedRow::dense(row));
            return if let Some(constraint) = constraint {
                self.scan_constrained(table, constraint, &mut dense)
            } else {
                self.scan(table, &mut dense)
            };
        }
        let entry = self.entry(table)?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
        if let Some(snapshot) = entry.table.get() {
            return self.visit_projection(snapshot, Some(indices), visitor);
        }
        let cached = entry
            .projected
            .try_borrow()
            .map_err(|_| QueryError::Source)?;
        for projection in cached.iter() {
            self.budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?
                .charge(Resource::Steps, (indices.len() + 1) as u64)
                .map_err(|error| query_error(error.into()))?;
            if indices
                .iter()
                .all(|index| projection.indices.binary_search(index).is_ok())
            {
                if indices == projection.indices {
                    return self.visit_projection(&projection.snapshot, None, visitor);
                }
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                budget
                    .charge_allocation(indices.len() as u64, size_of::<usize>() as u64)
                    .map_err(|error| query_error(error.into()))?;
                let mut positions = Vec::new();
                positions
                    .try_reserve_exact(indices.len())
                    .map_err(|_| QueryError::Allocation)?;
                for index in indices {
                    positions.push(
                        projection
                            .indices
                            .binary_search(index)
                            .map_err(|_| QueryError::Source)?,
                    );
                }
                drop(budget);
                return self.visit_projection(&projection.snapshot, Some(&positions), visitor);
            }
        }
        drop(cached);
        let (snapshot, complete) = self.projected_snapshot(entry, declared, indices, constraint)?;
        if !declared.matches_projection(&snapshot, indices) {
            return Err(QueryError::SchemaChanged);
        }
        if complete {
            tracing::debug!(
                rows = snapshot.rows().len(),
                projected_columns = indices.len(),
                "native projected scan completed"
            );
        } else {
            tracing::debug!(
                rows = snapshot.rows().len(),
                projected_columns = indices.len(),
                "native selected scan completed"
            );
        }
        if complete {
            let mut cache = entry
                .projected
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            budget
                .charge_allocation(indices.len() as u64, size_of::<usize>() as u64)
                .map_err(|error| query_error(error.into()))?;
            budget
                .charge_allocation(1, size_of::<CachedProjection>() as u64)
                .map_err(|error| query_error(error.into()))?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(indices.len())
                .map_err(|_| QueryError::Allocation)?;
            owned.extend_from_slice(indices);
            cache.try_reserve(1).map_err(|_| QueryError::Allocation)?;
            cache.push(CachedProjection {
                indices: owned,
                snapshot,
            });
            drop(budget);
            let position = cache.len() - 1;
            drop(cache);
            let cache = entry
                .projected
                .try_borrow()
                .map_err(|_| QueryError::Source)?;
            self.visit_projection(
                &cache.get(position).ok_or(QueryError::Source)?.snapshot,
                None,
                visitor,
            )
        } else {
            self.visit_projection(&snapshot, None, visitor)
        }
    }
    fn visit_projection(
        &self,
        snapshot: &TableSnapshot,
        indices: Option<&[usize]>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        for row in snapshot.rows() {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            visitor(match indices {
                Some(indices) => ProjectedRow::select(row, indices)?,
                None => ProjectedRow::dense(row),
            })?;
        }
        Ok(())
    }
    fn projected_snapshot(
        &self,
        entry: &Entry,
        declared: &ScalarSchema,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
    ) -> Result<(TableSnapshot, bool), QueryError> {
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
        if let Some(index) = &entry.index {
            let selection = if let Some(constraint) = constraint {
                let inspection = inspect_table_indexes(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?;
                projected_selection(&inspection, index.as_str(), declared, constraint)?
            } else {
                IndexProjectionSelection::All
            };
            let complete = selection == IndexProjectionSelection::All;
            let snapshot = read_table_index_projection(
                &mut *source,
                self.scope,
                &entry.name,
                index.as_str(),
                IndexProjection {
                    columns: &names,
                    selection,
                },
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            Ok((snapshot, complete))
        } else {
            Ok((
                read_table_projection_batched(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &names,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?,
                true,
            ))
        }
    }
}
