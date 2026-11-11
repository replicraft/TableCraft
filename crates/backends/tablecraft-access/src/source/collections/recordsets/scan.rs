//! Structured and scalar member projections share one checked parent visit.
use super::{NativeSource, allocate, check, copy, recordset_error};
use std::sync::Arc;
use tablecraft_ace::NativeReadSource;
use tablecraft_db::{
    MemberAvailability, OwnedRecordset, RecordsetPresence, StoredCell, validate_projection,
};
use tablecraft_query::{QueryError, SourceFailure, source::StructuredVisitor};

const ARC_COUNTERS: usize = 2;

impl<R: NativeReadSource> NativeSource<'_, R> {
    pub(in crate::source) fn structured_scan(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        recordsets: &[usize],
        visitor: &mut StructuredVisitor<'_>,
    ) -> Result<(), QueryError> {
        validate_projection(indices, self.schema_columns(table)?.len())?;
        validate_projection(values, self.collection_columns(table)?.len())?;
        validate_projection(members, self.named_members(table)?.len())?;
        validate_projection(recordsets, self.structured_columns(table)?.len())?;
        if recordsets.is_empty() {
            if values.is_empty() && members.is_empty() {
                return self
                    .projected_scan(table, indices, None, &mut |row| visitor(row, &[], &[]));
            }
            return self.member_scan(table, indices, values, members, &mut |row, lists| {
                visitor(row, lists, &[])
            });
        }
        let cache = self
            .entry(table)?
            .collections
            .get()
            .ok_or(QueryError::Source)?;
        let declarations = cache.recordsets.get().ok_or(QueryError::Source)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
        let mut combined = allocate(cache.members.len(), &mut budget)?;
        combined.extend_from_slice(members);
        for index in recordsets {
            check(&mut budget, self.cancelled)?;
            // Include each representative once within the paid full member width.
            // Its logical ROW is mapped by the same reader as explicit members.
            let group = &declarations.groups[*index];
            let owner = group
                .members
                .iter()
                .zip(declarations.columns[*index].schema().availability())
                .find_map(|(member, availability)| {
                    (*availability == MemberAvailability::Available).then_some(*member)
                })
                .ok_or(QueryError::Source)?;
            let mut position = 0;
            while position < combined.len() {
                check(&mut budget, self.cancelled)?;
                if combined[position] >= owner {
                    break;
                }
                position += 1;
            }
            if combined.get(position) != Some(&owner) {
                budget
                    .charge(
                        tablecraft_core::limits::Resource::Steps,
                        (combined.len() - position) as u64,
                    )
                    .map_err(crate::source::query_limit)?;
                combined.insert(position, owner);
            }
        }
        let width = values
            .len()
            .checked_add(members.len())
            .ok_or(QueryError::Allocation)?;
        let mut selected = allocate(width, &mut budget)?;
        selected.extend(0..values.len());
        for member in members {
            check(&mut budget, self.cancelled)?;
            budget
                .charge(
                    tablecraft_core::limits::Resource::Steps,
                    combined.len() as u64,
                )
                .map_err(crate::source::query_limit)?;
            selected.push(
                values.len()
                    + combined
                        .binary_search(member)
                        .map_err(|_| QueryError::Source)?,
            );
        }
        drop(budget);
        self.member_scan_with_parent(
            table,
            indices,
            values,
            &combined,
            &mut |row, lists, parent| {
                // Required-index qualification and the representative metadata
                // snapshot finish before any owned recordset is constructed.
                for index in recordsets {
                    self.load_recordset(table, *index)?;
                }
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
                check(&mut budget, self.cancelled)?;
                let mut exposed = allocate(selected.len(), &mut budget)?;
                for index in &selected {
                    exposed.push(*lists.get(*index).ok_or(QueryError::Source)?);
                }
                let mut records = allocate(recordsets.len(), &mut budget)?;
                for index in recordsets {
                    check(&mut budget, self.cancelled)?;
                    let stored = declarations.groups[*index]
                        .rows
                        .get()
                        .ok_or(QueryError::Source)?;
                    let value = stored.get(parent).ok_or(QueryError::Source)?;
                    // NO_CLONE: The source cache and query result independently retain
                    // the same immutable parent recordset; child payloads are not copied.
                    records.push(Arc::clone(value));
                }
                drop(budget);
                visitor(row, &exposed, &records)
            },
        )
    }

    fn load_recordset(&self, table: &str, index: usize) -> Result<(), QueryError> {
        let cache = self
            .entry(table)?
            .collections
            .get()
            .ok_or(QueryError::Source)?;
        let declarations = cache.recordsets.get().ok_or(QueryError::Source)?;
        let descriptor = declarations.columns.get(index).ok_or(QueryError::Source)?;
        let group = declarations.groups.get(index).ok_or(QueryError::Source)?;
        if group.rows.get().is_some() {
            return Ok(());
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
        let mut selected = allocate(group.members.len(), &mut budget)?;
        for (index, availability) in group.members.iter().zip(descriptor.schema().availability()) {
            check(&mut budget, self.cancelled)?;
            if *availability == MemberAvailability::Available {
                selected.push(*index);
            }
        }
        drop(budget);
        let fields = self.load_members(table, &selected)?;
        let parents = fields.first().ok_or(QueryError::Source)?.parents();
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
        let mut records = allocate(parents.len(), &mut budget)?;
        for (parent, membership) in parents.iter().enumerate() {
            check(&mut budget, self.cancelled)?;
            for field in &fields {
                check(&mut budget, self.cancelled)?;
                let link = field.parents().get(parent).ok_or(QueryError::Source)?;
                if field.parents().len() != parents.len()
                    || link.row_reference() != membership.row_reference()
                    || link.key() != membership.key()
                    || link.child_count() != membership.child_count()
                {
                    return Err(QueryError::Source);
                }
            }
            let mut rows = allocate(membership.child_count(), &mut budget)?;
            for child in 0..membership.child_count() {
                check(&mut budget, self.cancelled)?;
                let mut row = allocate(group.members.len(), &mut budget)?;
                let mut field = 0;
                for availability in descriptor.schema().availability() {
                    check(&mut budget, self.cancelled)?;
                    if *availability == MemberAvailability::Uninspected {
                        row.push(StoredCell::Uninspected);
                    } else {
                        let value = fields
                            .get(field)
                            .ok_or(QueryError::Source)?
                            .value(parent, child)
                            .ok_or(QueryError::Source)?;
                        row.push(StoredCell::Value(copy::value(
                            value,
                            &mut budget,
                            self.cancelled,
                        )?));
                        field += 1;
                    }
                }
                rows.push(row);
            }
            let presence = if membership.key().is_some() {
                RecordsetPresence::Assigned
            } else {
                RecordsetPresence::Unassigned
            };
            // NO_CLONE: Each immutable recordset and the independently retained
            // source declaration share one schema allocation, including empty parents.
            let record = OwnedRecordset::new(
                Arc::clone(descriptor.schema()),
                rows,
                presence,
                &mut budget,
                self.cancelled,
            )
            .map_err(recordset_error)?;
            budget
                .charge_allocation(
                    1,
                    (size_of::<OwnedRecordset>() + ARC_COUNTERS * size_of::<usize>()) as u64,
                )
                .map_err(crate::source::query_limit)?;
            records.push(Arc::new(record));
        }
        check(&mut budget, self.cancelled)?;
        group.rows.set(records).map_err(|_| QueryError::Source)
    }
}
