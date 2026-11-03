//! Complete original key census replay with selected-parent payload delivery.
use super::{Directory, Projection};
use crate::{
    InspectError, NativeReadSource, TableSnapshotLimits, format::reader::Reader,
    table::page::traversal::Traversal,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

/// Actual read inputs shared by both source-bound typed collection readers.
pub(in crate::table::complex) struct ParentRead<'a, R> {
    pub source: &'a mut R,
    pub scope: CatalogScope,
    pub limits: TableSnapshotLimits,
    pub budget: &'a mut ResourceBudget,
    pub cancelled: &'a dyn Fn() -> bool,
}

impl Directory {
    pub fn read_parent<R, T, P, F>(
        &self,
        read: ParentRead<'_, R>,
        parent: usize,
        payload: &Projection,
        prepare: P,
        mut retain: F,
    ) -> Result<T, InspectError>
    where
        R: NativeReadSource,
        P: FnOnce(usize, &mut ResourceBudget) -> Result<T, InspectError>,
        F: FnMut(&mut T, Vec<Value>, &mut ResourceBudget) -> Result<(), InspectError>,
    {
        self.require_source(
            read.source,
            read.scope,
            read.limits,
            read.budget,
            read.cancelled,
        )?;
        let count = self
            .parents()
            .get(parent)
            .ok_or(InspectError::InvalidPageRange)?
            .child_count();
        let mut result = prepare(count, read.budget)?;
        let schema = self.column().flat_table();
        let mut reader = Reader {
            source: read.source,
            header: schema.header,
            budget: read.budget,
            cancelled: read.cancelled,
            reads: 0,
        };
        let mut traversal = Traversal::new(&mut reader, schema)?;
        let mut observed = 0usize;
        let mut selected = 0usize;
        loop {
            if observed.is_multiple_of(read.limits.batch_rows() as usize) {
                // Reset only the predetermined physical batch quota. The actual
                // caller's allocation, row and work pools remain cumulative.
                reader.reads = 0;
            }
            let Some(root) = traversal.next(&mut reader, schema)? else {
                break;
            };
            let expected = self.children.get(observed).ok_or(InspectError::Malformed)?;
            if root.row_reference()? != expected.reference {
                return Err(InspectError::Malformed);
            }
            let target = self.membership.child_ordinal(parent, selected) == Some(observed);
            let projection = if target { payload } else { &self.keys };
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
                retain(&mut result, row, reader.budget)?;
                selected = selected
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
            observed = observed
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
        }
        reader.tick()?;
        if observed != self.children.len() || selected != count {
            return Err(InspectError::Malformed);
        }
        reader.tick()?;
        Ok(result)
    }
}
