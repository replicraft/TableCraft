//! Explicit normal row policy, separate from reusable physical serialization.
use crate::{
    EditError, InspectError, RowInsert, TableSchema, WriteBlocker,
    format::data_type::{LONG_TEXT, OLE},
    header::check,
    table::mutation::record_insertion,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(super) fn qualify(
    schema: &TableSchema,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    qualify_with(schema, requests, false, budget, cancelled)
}

/// TCNA4 declaration admission. Legacy protocols never call this entry.
pub(super) fn qualify_complete(
    schema: &TableSchema,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    qualify_with(schema, requests, true, budget, cancelled)
}

fn qualify_with(
    schema: &TableSchema,
    requests: &[RowInsert],
    complete: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // Complete declaration validation also runs for an empty batch. Generated
    // fields cannot become explicit manual targets through supplied NULL values.
    record_insertion::layout::variables_actions(schema, budget, cancelled)?;
    for field in schema.columns() {
        check(budget, cancelled)?;
        let wider = complete
            && matches!(
                field.scalar_type(),
                Some(
                    ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Single
                        | ScalarType::Double
                        | ScalarType::Guid
                        | ScalarType::Decimal
                )
            );
        if !wider
            && !matches!(field.native_type(), LONG_TEXT | OLE)
            && !matches!(
                field.scalar_type(),
                Some(
                    ScalarType::Boolean
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::DateTime
                        | ScalarType::Text
                        | ScalarType::Binary
                )
            )
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for (field, value) in schema.columns().iter().zip(request.values()) {
            check(budget, cancelled)?;
            let supported = match field.native_type() {
                LONG_TEXT => matches!(value, Value::Null | Value::Text(_)),
                OLE => matches!(value, Value::Null | Value::Binary(_)),
                _ => true,
            };
            if !supported {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn encode(
    schema: &TableSchema,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let variables = record_insertion::layout::variables_actions(schema, budget, cancelled)?;
    budget
        .charge_allocation(requests.len() as u64, size_of::<Vec<u8>>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    for request in requests {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        rows.push(record_insertion::build::build_with(
            schema,
            &variables,
            request.values(),
            budget,
            cancelled,
        )?);
    }
    check(budget, cancelled)?;
    Ok(rows)
}
