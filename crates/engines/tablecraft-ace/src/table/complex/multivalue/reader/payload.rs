//! Full key census replay, with Value decoding only for the requested parent.
use super::{MultivalueReader, ParentMultivalue};
use crate::{
    InspectError, NativeReadSource, format::reader::Reader, table::page::traversal::Traversal,
};
use tablecraft_core::values::Value;

impl<R: NativeReadSource> MultivalueReader<'_, R> {
    pub(super) fn read_payload(&mut self, parent: usize) -> Result<ParentMultivalue, InspectError> {
        self.directory.require_source(
            self.source,
            self.scope,
            self.limits,
            self.budget,
            self.cancelled,
        )?;
        let link = self
            .directory
            .parents()
            .get(parent)
            .ok_or(InspectError::InvalidPageRange)?;
        let count = link.child_count();
        self.budget
            .charge_allocation(count as u64, size_of::<Value>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        let schema = self.directory.column().flat_table();
        let mut reader = Reader {
            source: &mut *self.source,
            header: schema.header,
            budget: self.budget,
            cancelled: self.cancelled,
            reads: 0,
        };
        let mut traversal = Traversal::new(&mut reader, schema)?;
        let mut observed = 0usize;
        let mut selected = 0usize;
        loop {
            if observed.is_multiple_of(self.limits.batch_rows() as usize) {
                // A predetermined key-record batch resets only the physical read
                // quota, never the original caller's cumulative resource pools.
                reader.reads = 0;
            }
            let Some(root) = traversal.next(&mut reader, schema)? else {
                break;
            };
            let expected = self
                .directory
                .children
                .get(observed)
                .ok_or(InspectError::Malformed)?;
            if root.row_reference()? != expected.reference {
                return Err(InspectError::Malformed);
            }
            let target =
                self.directory.membership.child_ordinal(parent, selected) == Some(observed);
            let projection = if target {
                &self.values
            } else {
                &self.directory.keys
            };
            let row = traversal.values(
                &root,
                &mut reader,
                schema,
                &projection.columns,
                Some(&projection.positions),
            )?;
            reader.work(projection.roles.len())?;
            if !matches!(row.get(projection.roles[0]), Some(Value::Int32(value)) if *value == expected.foreign_key)
                || !matches!(row.get(projection.roles[1]), Some(Value::Int32(value)) if *value == expected.identity)
            {
                return Err(InspectError::Malformed);
            }
            if target {
                let value = row
                    .into_iter()
                    .nth(projection.roles[2])
                    .ok_or(InspectError::Malformed)?;
                values.push(value);
                selected = selected
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
            observed = observed
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
        }
        reader.tick()?;
        if observed != self.directory.children.len() || selected != count {
            return Err(InspectError::Malformed);
        }
        reader.tick()?;
        Ok(ParentMultivalue {
            reference: link.row_reference(),
            key: link.key(),
            values,
        })
    }
}
