//! Independent metadata ownership is paid before fallible, cancellable payload copies.
use super::{check, query_error};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_query::{QueryError, SourceFailure};
use tablecraft_schema::Column;

const COPY_BYTES: usize = 4096;

pub(super) fn text(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, QueryError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(value.len() as u64, 1)
        .map_err(crate::source::query_limit)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
    let mut offset = 0;
    while offset < value.len() {
        check(budget, cancelled)?;
        let mut end = offset.saturating_add(COPY_BYTES).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        budget
            .charge(Resource::Steps, (end - offset) as u64)
            .map_err(crate::source::query_limit)?;
        output.push_str(&value[offset..end]);
        offset = end;
    }
    Ok(output)
}

pub(super) fn column(
    value: &Column,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Column, QueryError> {
    Column::with_type(
        text(value.name(), budget, cancelled)?,
        value.type_descriptor(),
        value.nullable(),
    )
    .map_err(|_| QueryError::SchemaChanged)
}

pub(super) fn value(
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, QueryError> {
    check(budget, cancelled)?;
    Ok(match value {
        Value::Null => Value::Null,
        Value::Boolean(value) => Value::Boolean(*value),
        Value::Byte(value) => Value::Byte(*value),
        Value::Int16(value) => Value::Int16(*value),
        Value::Int32(value) => Value::Int32(*value),
        Value::Int64(value) => Value::Int64(*value),
        Value::Single(value) => Value::Single(*value),
        Value::Double(value) => Value::Double(*value),
        Value::Currency(value) => Value::Currency(*value),
        Value::Decimal(value) => Value::Decimal(*value),
        Value::Guid(value) => Value::Guid(*value),
        Value::DateTime(value) => Value::DateTime(*value),
        Value::DateTimeExtended(value) => Value::DateTimeExtended(*value),
        Value::Text(value) => Value::Text(text(value, budget, cancelled)?),
        // FileData is declared but never requested by the metadata view.
        Value::Binary(_) => return Err(query_error(tablecraft_ace::InspectError::Malformed)),
    })
}
