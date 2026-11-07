//! Original presence, exact supplied carriers and stored constant companion defaults.
use crate::{
    EditError, InsertFieldValue, InspectError, NativeColumn, NativeImage, OmittedRowInsert,
    RowInsert, TableProperties, TableSchema, WriteBlocker,
    format::data_type::{
        BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, NUMERIC,
        SHORT_BINARY, SHORT_TEXT, SINGLE,
    },
    header::check,
    inspect_table_properties,
    table::mutation::{
        autonumber_counter,
        insert_defaults::{declaration, literal, values::copy_value},
        preserved_source::Source,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) fn validate(
    image: &NativeImage,
    source: &Source,
    column: usize,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.table.schema();
    let properties = capture_properties(image, schema, budget, cancelled)?;

    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for (position, (field, original)) in
            schema.columns().iter().zip(request.values()).enumerate()
        {
            check(budget, cancelled)?;
            if position == column {
                if !matches!(original, InsertFieldValue::Omitted) {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                continue;
            }
            let default;
            let value = match original {
                InsertFieldValue::Supplied(value) => value,
                InsertFieldValue::Omitted => {
                    default = resolve_default(field, &properties, budget, cancelled)?;
                    &default
                }
            };
            let valid = matches!(
                (field.native_type(), value),
                (BOOLEAN, Value::Boolean(_))
                    | (BYTE, Value::Byte(_) | Value::Null)
                    | (INT16, Value::Int16(_) | Value::Null)
                    | (SINGLE, Value::Single(_) | Value::Null)
                    | (DOUBLE, Value::Double(_) | Value::Null)
                    | (GUID, Value::Guid(_) | Value::Null)
                    | (NUMERIC, Value::Decimal(_) | Value::Null)
                    | (INT32, Value::Int32(_) | Value::Null)
                    | (INT64, Value::Int64(_) | Value::Null)
                    | (CURRENCY, Value::Currency(_) | Value::Null)
                    | (DATE_TIME, Value::DateTime(_) | Value::Null)
                    | (SHORT_TEXT, Value::Text(_) | Value::Null)
                    | (SHORT_BINARY, Value::Binary(_) | Value::Null)
            );
            let finite = match value {
                Value::Single(value) => f32::from_bits(value.to_bits()).is_finite(),
                Value::Double(value) => f64::from_bits(value.to_bits()).is_finite(),
                _ => true,
            };
            if !valid || !finite {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            if schema.layout.physical_indexes > 1 {
                for index in source.indexes.indexes() {
                    check(budget, cancelled)?;
                    if index
                        .columns()
                        .iter()
                        .any(|key| key.number() == field.number())
                        && !matches!(value, Value::Int32(_))
                    {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                }
            }
        }
    }

    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn expand(
    image: &NativeImage,
    source: &Source,
    generated: &autonumber_counter::Plan,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    if generated.values.len() != requests.len() {
        return Err(EditError::Preservation);
    }

    let properties = capture_properties(image, source.table.schema(), budget, cancelled)?;
    let mut rows = super::vector(requests.len(), budget, cancelled)?;
    for (request, generated_value) in requests.iter().zip(&generated.values) {
        check(budget, cancelled)?;
        let mut values = super::vector(request.values().len(), budget, cancelled)?;
        for (position, original) in request.values().iter().enumerate() {
            check(budget, cancelled)?;
            if position == generated.source.column {
                if !matches!(original, InsertFieldValue::Omitted) {
                    return Err(EditError::Preservation);
                }
                values.push(Value::Int32(*generated_value));
            } else {
                let value = match original {
                    InsertFieldValue::Supplied(value) => copy_value(value, budget, cancelled)?,
                    InsertFieldValue::Omitted => resolve_default(
                        &source.table.schema().columns()[position],
                        &properties,
                        budget,
                        cancelled,
                    )?,
                };
                values.push(value);
            }
        }
        rows.push(RowInsert::new(values));
    }

    check(budget, cancelled)?;
    Ok(rows)
}

pub(super) fn capture_properties(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableProperties, EditError> {
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    budget
        .charge(Resource::Steps, schema.name().len() as u64)
        .map_err(InspectError::from)?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(EditError::Preservation);
    }
    Ok(properties)
}

fn resolve_default(
    column: &NativeColumn,
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    match declaration::capture(column, properties, budget, cancelled)? {
        Some(property) => literal::resolve(column, property.text()?, budget, cancelled),
        // Complete inspected absence supplies NULL, never a fabricated default.
        // Carrier, index and actual Required checks still qualify the result.
        None => Ok(Value::Null),
    }
}
