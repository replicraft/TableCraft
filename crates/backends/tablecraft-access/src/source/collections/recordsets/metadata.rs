//! Member origin is retained from actual attachment discovery, never guessed from names.
use super::{Group, NativeSource, Recordsets, allocate, check, copy, recordset_error};
use crate::source::collections::payload;
use std::{cell::OnceCell, sync::Arc};
use tablecraft_ace::NativeReadSource;
use tablecraft_db::{MemberAvailability, RecordsetSchema};
use tablecraft_query::{QueryError, source::RecordsetColumn};
use tablecraft_schema::NameComparison;

const ARC_COUNTERS: usize = 2;

impl<R: NativeReadSource> NativeSource<'_, R> {
    pub(in crate::source) fn structured_columns(
        &self,
        table: &str,
    ) -> Result<&[RecordsetColumn], QueryError> {
        if !self.collections {
            return Ok(&[]);
        }
        self.named_members(table)?;
        let cache = self
            .entry(table)?
            .collections
            .get()
            .ok_or(QueryError::Source)?;
        if cache.recordsets.get().is_none() {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::SourceFailure(tablecraft_query::SourceFailure::Busy))?;
            let mut columns = allocate(cache.attachment_member_count, &mut budget)?;
            let mut groups: Vec<Group> = allocate(cache.attachment_member_count, &mut budget)?;
            for owner in 0..cache.attachment_member_count {
                check(&mut budget, self.cancelled)?;
                let member = &cache.members[owner];
                let mut seen = false;
                for group in &groups {
                    check(&mut budget, self.cancelled)?;
                    seen |= cache.members[group.owner].collection() == member.collection();
                }
                if seen {
                    continue;
                }
                let mut count = 0;
                for other in &cache.members[..cache.attachment_member_count] {
                    check(&mut budget, self.cancelled)?;
                    if other.collection() == member.collection() {
                        count += 1;
                    }
                }
                let mut indices = allocate(count, &mut budget)?;
                let mut child_columns = allocate(count, &mut budget)?;
                let mut availability = allocate(count, &mut budget)?;
                for (index, other) in cache.members[..cache.attachment_member_count]
                    .iter()
                    .enumerate()
                {
                    check(&mut budget, self.cancelled)?;
                    if other.collection() != member.collection() {
                        continue;
                    }
                    indices.push(index);
                    child_columns.push(copy::column(other.column(), &mut budget, self.cancelled)?);
                    availability.push(
                        if other
                            .column()
                            .name()
                            .eq_ignore_ascii_case(payload::FILE_DATA)
                        {
                            MemberAvailability::Uninspected
                        } else {
                            MemberAvailability::Available
                        },
                    );
                }
                let schema = RecordsetSchema::new(
                    child_columns,
                    availability,
                    NameComparison::AsciiInsensitive,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(recordset_error)?;
                budget
                    .charge_allocation(
                        1,
                        (size_of::<RecordsetSchema>() + ARC_COUNTERS * size_of::<usize>()) as u64,
                    )
                    .map_err(crate::source::query_limit)?;
                let name = copy::text(member.collection().as_str(), &mut budget, self.cancelled)?;
                columns.push(
                    RecordsetColumn::new(name, Arc::new(schema))
                        .map_err(|_| QueryError::SchemaChanged)?,
                );
                groups.push(Group {
                    owner,
                    members: indices,
                    rows: OnceCell::new(),
                });
            }
            check(&mut budget, self.cancelled)?;
            cache
                .recordsets
                .set(Recordsets { columns, groups })
                .map_err(|_| QueryError::Source)?;
        }
        Ok(&cache.recordsets.get().ok_or(QueryError::Source)?.columns)
    }
}
