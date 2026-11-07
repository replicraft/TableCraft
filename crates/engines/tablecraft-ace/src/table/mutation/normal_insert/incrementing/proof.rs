//! Fresh original presence and independent signed arithmetic; no staged keys.
use crate::{
    EditError, InsertFieldValue, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    RowInsert, WriteBlocker,
    format::data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64, SHORT_BINARY, SHORT_TEXT},
    header::check,
    table::mutation::{
        insert_defaults::{declaration, proof_literal, values::copy_value},
        record_insertion::properties,
    },
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
    let defaults =
        super::binding::capture_properties(&image, source.table.schema(), budget, cancelled)?;

    // Derive from the exact original word with wider arithmetic independently
    // of the producer's counter::prepare vector and checked-add loop.
    let delta = i64::try_from(original.len()).map_err(|_| EditError::AutoNumberExhausted)?;
    let final_last = i32::try_from(i64::from(counter.last) + delta)
        .map_err(|_| EditError::AutoNumberExhausted)?;
    let mut rows = super::vector(original.len(), budget, cancelled)?;
    for (ordinal, request) in original.iter().enumerate() {
        check(budget, cancelled)?;
        if request.values().len() != source.table.schema().columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let step = i64::try_from(ordinal + 1).map_err(|_| EditError::AutoNumberExhausted)?;
        let value = i32::try_from(i64::from(counter.last) + step)
            .map_err(|_| EditError::AutoNumberExhausted)?;
        let mut values = super::vector(request.values().len(), budget, cancelled)?;
        for (position, (field, original)) in source
            .table
            .schema()
            .columns()
            .iter()
            .zip(request.values())
            .enumerate()
        {
            check(budget, cancelled)?;
            if position == counter.column {
                if !matches!(original, InsertFieldValue::Omitted) {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                values.push(Value::Int32(value));
            } else {
                let derived;
                let supplied = match original {
                    InsertFieldValue::Supplied(value) => value,
                    InsertFieldValue::Omitted => {
                        let property = declaration::capture(field, &defaults, budget, cancelled)?
                            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
                        let mut used_clock = false;
                        derived = proof_literal::derive(
                            field,
                            property.text()?,
                            None,
                            &mut used_clock,
                            budget,
                            cancelled,
                        )?;
                        if used_clock {
                            return Err(EditError::Preservation);
                        }
                        &derived
                    }
                };
                match (field.native_type(), supplied) {
                    (BOOLEAN, Value::Boolean(_))
                    | (INT32, Value::Int32(_) | Value::Null)
                    | (INT64, Value::Int64(_) | Value::Null)
                    | (CURRENCY, Value::Currency(_) | Value::Null)
                    | (DATE_TIME, Value::DateTime(_) | Value::Null)
                    | (SHORT_TEXT, Value::Text(_) | Value::Null)
                    | (SHORT_BINARY, Value::Binary(_) | Value::Null) => {}
                    _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
                }
                values.push(copy_value(supplied, budget, cancelled)?);
            }
        }
        rows.push(RowInsert::new(values));
    }
    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
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
    if source.table.schema().layout.physical_indexes == 1 {
        let index = source
            .indexes
            .indexes()
            .first()
            .ok_or(EditError::Preservation)?;
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
    } else {
        for physical in 0..source.table.schema().layout.physical_indexes {
            check(budget, cancelled)?;
            let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
            let index = crate::table::indexes::mutation::insertion::qualify_actions(
                facts.checked(),
                physical,
                budget,
                cancelled,
            )?;
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
        }
    }
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
