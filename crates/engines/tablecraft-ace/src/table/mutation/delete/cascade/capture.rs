//! Original ordinary-root identities and exact raw records, using the shared walker.
use super::{EditError, RowDelete, reserve};
use crate::{
    InspectError, NativeImage, TableSchema, format::reader::Reader, header::check,
    table::mutation::rows,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Ordinal(pub usize);

impl rows::RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) fn references(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let count = schema.declared_rows() as usize;
    let mut ordinals = Vec::new();
    let mut references = Vec::new();
    for row in 0..count {
        check(budget, cancelled)?;
        reserve(&mut ordinals, budget)?;
        ordinals.push(Ordinal(row));
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    rows::visit_with_budget(&mut reader, schema, &ordinals, |budget, row, _| {
        reserve(&mut references, budget)?;
        references.push(row.reference);
        Ok(())
    })?;
    Ok(references)
}

pub(super) fn records(
    image: &NativeImage,
    schema: &TableSchema,
    selected: &[bool],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowDelete>, EditError> {
    let mut ordinals = Vec::new();
    for (row, &included) in selected.iter().enumerate() {
        check(budget, cancelled)?;
        if included {
            reserve(&mut ordinals, budget)?;
            ordinals.push(Ordinal(row));
        }
    }
    // Pay both the request vector and each owned record before allocation.
    budget
        .charge_allocation(ordinals.len() as u64, size_of::<RowDelete>() as u64)
        .map_err(InspectError::from)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(ordinals.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    rows::visit_with_budget(&mut reader, schema, &ordinals, |budget, row, ordinal| {
        budget
            .charge_allocation(row.data.len() as u64, 1)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(row.data.len())
            .map_err(|_| InspectError::Allocation)?;
        bytes.extend_from_slice(row.data);
        output.push(RowDelete::new(ordinal.0, bytes));
        Ok(())
    })?;
    Ok(output)
}
