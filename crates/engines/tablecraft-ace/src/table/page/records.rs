//! Complete batched records for feature-qualified carriers borrowed from a schema.
use super::{TableSnapshotLimits, traversal::Traversal};
use crate::{
    InspectError,
    format::reader::Reader,
    table::{NativeRowReference, TableSchema},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::Column;

// Only owning feature readers may supply nonordinary scalar projections here.
// Public ordinary projections must retain their separate carrier qualification.
pub(in crate::table) fn read_records_batched<R: crate::NativeReadSource>(
    source: &mut R,
    schema: &TableSchema,
    columns: &[Column],
    selected: Option<&[usize]>,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<(NativeRowReference, Vec<Value>)>, InspectError> {
    if schema.declared_rows() > limits.maximum_rows() {
        return Err(InspectError::StructureLimit);
    }
    let mut traversal = {
        let mut reader = Reader {
            source: &mut *source,
            header: schema.header,
            budget,
            cancelled,
            reads: 0,
        };
        Traversal::new(&mut reader, schema)?
    };
    let mut rows = Vec::new();
    loop {
        let mut reader = Reader {
            source: &mut *source,
            header: schema.header,
            budget,
            cancelled,
            reads: 0,
        };
        for _ in 0..limits.batch_rows() {
            let Some(root) = traversal.next(&mut reader, schema)? else {
                break;
            };
            let values = traversal.values(&root, &mut reader, schema, columns, selected)?;
            let reference = root.row_reference()?;
            reader
                .budget
                .charge_allocation(1, size_of::<(NativeRowReference, Vec<Value>)>() as u64)?;
            rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            rows.push((reference, values));
        }
        reader.tick()?;
        if traversal.ended() {
            return Ok(rows);
        }
    }
}
