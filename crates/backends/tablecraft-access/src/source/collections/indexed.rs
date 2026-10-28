//! Required-index parents joined to complete collection snapshots by logical ROW.
use super::{NativeField, NativeList, NativeSource, query_error};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, read_stored_table_index_with_references,
    read_table_index_with_references,
};
use tablecraft_core::limits::Resource;
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_query::{QueryError, source::MultivalueVisitor};

impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn indexed_member_scan(
        &self,
        table: &str,
        indices: &[usize],
        fields: &[NativeField<'_>],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let entry = self.entry(table)?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
        let index = entry.index.as_ref().ok_or(QueryError::Source)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        let names = declared
            .selected_names(indices, &mut budget)
            .map_err(query_error)?;
        let mut source = self
            .source
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        let read_index = if declared.has_cached(Some(indices)) {
            read_stored_table_index_with_references
        } else {
            read_table_index_with_references
        };
        let indexed = read_index(
            &mut *source,
            self.scope,
            &entry.name,
            index.as_str(),
            IndexReadRequest {
                columns: Some(&names),
                selection: IndexProjectionSelection::All,
                maximum_rows: self.snapshot_limits.maximum_rows(),
            },
            &mut budget,
            self.cancelled,
        )
        .map_err(query_error)?;
        drop(source);

        let snapshot = indexed.snapshot();
        self.check_snapshot_rows(snapshot)?;
        if !declared.matches_projection(snapshot, indices) {
            return Err(QueryError::SchemaChanged);
        }
        let parents = fields.first().ok_or(QueryError::Source)?.parents();
        if parents.len() != snapshot.rows().len()
            || indexed.row_references().len() != snapshot.rows().len()
        {
            return Err(QueryError::Source);
        }

        // These snapshots all came from this NativeSource's stable/offline handle.
        // Metadata checks do not grant identity across unrelated database handles.
        for field in fields {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            budget.charge(
                Resource::Steps,
                snapshot.schema().columns().len() as u64 + 1,
            )?;
            if !snapshot.schema().same_definition(field.owner()) {
                return Err(QueryError::SchemaChanged);
            }
            let links = field.parents();
            if links.len() != parents.len() {
                return Err(QueryError::Source);
            }
            let mut previous = None;
            for (position, parent) in links.iter().enumerate() {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                budget.charge(Resource::Steps, 1)?;
                let reference = parent.row_reference();
                if previous.is_some_and(|old| old >= reference)
                    || parents[position].row_reference() != reference
                {
                    return Err(QueryError::Source);
                }
                previous = Some(reference);
            }
        }

        budget.charge_allocation(parents.len() as u64, size_of::<u8>() as u64)?;
        budget.charge_allocation(parents.len() as u64, size_of::<usize>() as u64)?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(parents.len())
            .map_err(|_| QueryError::Allocation)?;
        seen.resize(parents.len(), 0_u8);
        let mut ordinals = Vec::new();
        ordinals
            .try_reserve_exact(parents.len())
            .map_err(|_| QueryError::Allocation)?;

        // Complete All scans must form a bijection before any visitor is called.
        for reference in indexed.row_references() {
            let mut start = 0;
            let mut end = parents.len();
            while start < end {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                budget.charge(Resource::Steps, 1)?;
                let middle = start + (end - start) / 2;
                if parents[middle].row_reference() < *reference {
                    start = middle + 1;
                } else {
                    end = middle;
                }
            }
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            budget.charge(Resource::Steps, 1)?;
            if parents.get(start).map(|parent| parent.row_reference()) != Some(*reference)
                || seen[start] != 0
            {
                return Err(QueryError::Source);
            }
            seen[start] = 1;
            ordinals.push(start);
        }
        drop(budget);

        for (row, parent) in snapshot.rows().iter().zip(ordinals) {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            budget.charge(Resource::Steps, fields.len() as u64 + 1)?;
            budget.charge_allocation(
                fields.len() as u64,
                (size_of::<NativeList<'_>>() + size_of::<&dyn MultivalueList>()) as u64,
            )?;
            let mut lists = Vec::new();
            lists
                .try_reserve_exact(fields.len())
                .map_err(|_| QueryError::Allocation)?;
            for field in fields {
                lists.push(NativeList {
                    field,
                    parent,
                    length: field.parents()[parent].child_count(),
                });
            }
            let mut views = Vec::new();
            views
                .try_reserve_exact(lists.len())
                .map_err(|_| QueryError::Allocation)?;
            views.extend(lists.iter().map(|list| list as &dyn MultivalueList));
            drop(budget);
            visitor(ProjectedRow::dense(row), &views)?;
        }
        Ok(())
    }
}
