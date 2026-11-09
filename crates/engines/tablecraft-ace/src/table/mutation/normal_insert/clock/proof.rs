//! Fresh original presence plus independently parsed default calls and full DATA replay.
use crate::table::mutation::insert_defaults::{declaration, proof_literal, values::copy_value};
use crate::{
    EditError, InsertFieldValue, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    RowInsert, header::check, inspect_table_properties,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[OmittedRowInsert],
    reading: Option<ClockReading>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != requests.len()
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
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    if !source.indexes.indexes().is_empty() {
        return Err(EditError::Preservation);
    }
    super::qualify(source.table.schema(), budget, cancelled)?;
    super::super::records::qualify(source.table.schema(), &[], budget, cancelled)?;
    let schema = source.table.schema();
    let mut omitted = false;
    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Preservation);
        }
        for field in request.values() {
            check(budget, cancelled)?;
            omitted |= matches!(field, InsertFieldValue::Omitted);
        }
    }
    let properties = if omitted {
        let properties =
            inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
        if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
            return Err(EditError::Preservation);
        }
        super::qualify_properties(schema, &properties, budget, cancelled)?;
        Some(properties)
    } else {
        None
    };
    let mut used_clock = false;
    let mut resolved = vector(requests.len(), budget, cancelled)?;
    for request in requests {
        check(budget, cancelled)?;
        let mut values = vector(schema.columns().len(), budget, cancelled)?;
        for (column, original) in schema.columns().iter().zip(request.values()) {
            check(budget, cancelled)?;
            let value = match original {
                InsertFieldValue::Supplied(value) => copy_value(value, budget, cancelled)?,
                InsertFieldValue::Omitted => {
                    match declaration::capture(
                        column,
                        properties.as_ref().ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )? {
                        Some(property) => proof_literal::derive(
                            column,
                            property.text()?,
                            reading,
                            &mut used_clock,
                            budget,
                            cancelled,
                        )?,
                        None => Value::Null,
                    }
                }
            };
            values.push(value);
        }
        resolved.push(RowInsert::new(values));
    }
    if used_clock != reading.is_some() {
        return Err(EditError::Preservation);
    }
    // Independent normal record/placement proof consumes only freshly derived
    // original values. No bound producer rows or candidate byte patches enter.
    super::super::proof::prove(plan, scope, table, &resolved, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}
