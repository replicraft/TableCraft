//! Original presence and exact supplied carriers for ordinary companions.
use crate::{
    EditError, InsertFieldValue, OmittedRowInsert, RowInsert, WriteBlocker,
    format::data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64, SHORT_BINARY, SHORT_TEXT},
    header::check,
    table::mutation::{
        autonumber_counter, insert_defaults::values::copy_value, preserved_source::Source,
    },
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) fn validate(
    source: &Source,
    column: usize,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.table.schema();

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
            let InsertFieldValue::Supplied(value) = original else {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            };
            let valid = matches!(
                (field.native_type(), value),
                (BOOLEAN, Value::Boolean(_))
                    | (INT32, Value::Int32(_) | Value::Null)
                    | (INT64, Value::Int64(_) | Value::Null)
                    | (CURRENCY, Value::Currency(_) | Value::Null)
                    | (DATE_TIME, Value::DateTime(_) | Value::Null)
                    | (SHORT_TEXT, Value::Text(_) | Value::Null)
                    | (SHORT_BINARY, Value::Binary(_) | Value::Null)
            );
            if !valid {
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
    generated: &autonumber_counter::Plan,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    if generated.values.len() != requests.len() {
        return Err(EditError::Preservation);
    }

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
                let InsertFieldValue::Supplied(value) = original else {
                    return Err(EditError::Preservation);
                };
                values.push(copy_value(value, budget, cancelled)?);
            }
        }
        rows.push(RowInsert::new(values));
    }

    check(budget, cancelled)?;
    Ok(rows)
}
