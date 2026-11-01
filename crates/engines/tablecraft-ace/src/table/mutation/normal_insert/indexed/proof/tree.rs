//! Fresh raw-row keys and increasing-cardinality NODE reconstruction.
use super::{RequestedRows, optional};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck,
    header::check,
    table::{indexes::mutation::normal_index_audit::Ownership, mutation::preserved_source::Source},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

type Key = (Option<i32>, u32);

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub source: &'a Source,
    pub checked: &'a TableCheck,
    pub index: &'a NativeIndex,
    pub rows: RequestedRows<'a>,
}

pub(super) fn reconstruct(
    operation: Operation<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let Operation {
        image,
        source,
        checked,
        index,
        rows,
    } = operation;
    let RequestedRows {
        requests,
        references,
    } = rows;
    if requests.len() != references.len() || source.records.len() != source.table.rows().len() {
        return Err(EditError::Preservation);
    }
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() {
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Preservation)?;
    let field = &schema.columns()[column];
    let mut original = super::super::vector(source.records.len(), budget, cancelled)?;
    for (record, row) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = optional::original_value(
            &record.data,
            schema,
            field,
            row.get(column).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        original.push((value, record.reference));
    }
    optional::sort(&mut original, key.descending(), budget, cancelled)?;
    let capacity = original
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut final_keys = super::super::vector(capacity, budget, cancelled)?;
    let copy_work = original
        .len()
        .checked_mul(size_of::<Key>())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, copy_work as u64)
        .map_err(InspectError::from)?;
    final_keys.extend_from_slice(&original);
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let value = match request.values().get(column) {
            Some(Value::Int32(value)) => Some(*value),
            Some(Value::Null) => None,
            _ => return Err(EditError::Preservation),
        };
        final_keys.push((value, *reference));
    }
    optional::sort(&mut final_keys, key.descending(), budget, cancelled)?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    for pair in &final_keys {
        check(budget, cancelled)?;
        if (constraints.required() && pair.0.is_none())
            || (constraints.unique() && pair.0.is_some() && previous == Some(pair.0))
        {
            return Err(EditError::Preservation);
        }
        previous = Some(pair.0);
    }
    let before_distinct = optional::distinct(&original, budget, cancelled)?;
    let after_distinct = optional::distinct(&final_keys, budget, cancelled)?;
    let original_wire = wires(&original, key.descending(), budget, cancelled)?;
    let final_wire = wires(&final_keys, key.descending(), budget, cancelled)?;
    let ownership = Ownership::capture(image, checked, budget, cancelled)?;
    let tree = ownership.require(image, schema, index, budget, cancelled)?;
    tree.reconstruct_insertions(
        image,
        &original_wire,
        &final_wire,
        expected,
        budget,
        cancelled,
    )?;
    super::reconstruct_history(
        image,
        source,
        index,
        (before_distinct, after_distinct),
        expected,
        budget,
        cancelled,
    )
}

fn wires(
    keys: &[Key],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let mut result = super::super::vector(keys.len(), budget, cancelled)?;
    for pair in keys {
        check(budget, cancelled)?;
        let (raw, length) = optional::wire(*pair, descending);
        let mut value = super::super::vector(length, budget, cancelled)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        value.extend_from_slice(&raw[..length]);
        result.push(value);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
