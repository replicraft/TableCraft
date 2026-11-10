//! Independent original-presence expansion and stored signed counter arithmetic.
use crate::{
    EditError, InsertFieldValue, NativeImage, OmittedRowInsert, RowInsert, WriteBlocker,
    format::data_type::{
        BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, NUMERIC,
        SHORT_BINARY, SHORT_TEXT, SINGLE,
    },
    header::check,
    table::mutation::{
        autonumber_counter,
        insert_defaults::{declaration, proof_literal, values::copy_value},
        preserved_source::Source,
    },
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(in crate::table::mutation::normal_insert) struct Derived {
    pub rows: Vec<RowInsert>,
    pub last: i32,
}

pub(in crate::table::mutation::normal_insert) fn derive(
    image: &NativeImage,
    source: &Source,
    counter: &autonumber_counter::Source,
    original: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Derived, EditError> {
    let defaults =
        super::binding::capture_properties(image, source.table.schema(), budget, cancelled)?;

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
                        derived = match declaration::capture(field, &defaults, budget, cancelled)? {
                            Some(property) => {
                                let mut used_clock = false;
                                let value = proof_literal::derive(
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
                                value
                            }
                            // Derive absence from fresh original metadata here;
                            // no expanded producer row supplies this NULL.
                            None => Value::Null,
                        };
                        &derived
                    }
                };
                match (field.native_type(), supplied) {
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
                    | (SHORT_BINARY, Value::Binary(_) | Value::Null) => {}
                    _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
                }
                match supplied {
                    Value::Single(value) if !f32::from_bits(value.to_bits()).is_finite() => {
                        return Err(EditError::Unsupported(WriteBlocker::Column));
                    }
                    Value::Double(value) if !f64::from_bits(value.to_bits()).is_finite() => {
                        return Err(EditError::Unsupported(WriteBlocker::Column));
                    }
                    _ => {}
                }
                values.push(copy_value(supplied, budget, cancelled)?);
            }
        }
        rows.push(RowInsert::new(values));
    }
    Ok(Derived {
        rows,
        last: final_last,
    })
}
