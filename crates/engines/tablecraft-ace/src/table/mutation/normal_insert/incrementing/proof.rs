//! Fresh original presence and independent signed arithmetic; no staged keys.
use crate::{
    EditError, InsertFieldValue, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    RowInsert, WriteBlocker, header::check,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

pub(in crate::table::mutation::normal_insert) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != original.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture_source(&image, table, budget, cancelled)?;
    let counter = super::mode::capture(&image, &source, budget, cancelled)?;
    // Derive from the exact original word with wider arithmetic independently
    // of the producer's counter::prepare vector and checked-add loop.
    let delta = i64::try_from(original.len()).map_err(|_| EditError::AutoNumberExhausted)?;
    let final_last = i32::try_from(i64::from(counter.last) + delta)
        .map_err(|_| EditError::AutoNumberExhausted)?;
    let mut rows = super::vector(original.len(), budget, cancelled)?;
    for (ordinal, request) in original.iter().enumerate() {
        check(budget, cancelled)?;
        let [
            InsertFieldValue::Omitted,
            InsertFieldValue::Supplied(Value::Int32(score)),
        ] = request.values()
        else {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        };
        let step = i64::try_from(ordinal + 1).map_err(|_| EditError::AutoNumberExhausted)?;
        let value = i32::try_from(i64::from(counter.last) + step)
            .map_err(|_| EditError::AutoNumberExhausted)?;
        let mut values = super::vector(2, budget, cancelled)?;
        values.push(Value::Int32(value));
        values.push(Value::Int32(*score));
        rows.push(RowInsert::new(values));
    }
    let (facts, optional) =
        super::super::indexed::qualify(&image, &source, table, &rows, budget, cancelled)?;
    if optional {
        return Err(EditError::Preservation);
    }
    let (mut expected, references) = super::super::proof::reconstruct_incrementing(
        &image,
        &source,
        &rows,
        counter.column,
        budget,
        cancelled,
    )?;
    let [index] = source.indexes.indexes() else {
        return Err(EditError::Preservation);
    };
    super::super::indexed::proof::reconstruct_index(
        &image,
        &source,
        index,
        super::super::indexed::proof::RequestedRows {
            requests: &rows,
            references: &references,
        },
        &mut expected,
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    expected[counter.offset..counter.offset + size_of::<i32>()]
        .copy_from_slice(&final_last.to_le_bytes());
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture_source(&after, table, budget, cancelled)?;
    let final_counter = super::mode::capture(&after, &final_source, budget, cancelled)?;
    let (final_facts, _) =
        super::super::indexed::qualify(&after, &final_source, table, &[], budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != source.records.len() + original.len()
        || final_counter.last != final_last
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
