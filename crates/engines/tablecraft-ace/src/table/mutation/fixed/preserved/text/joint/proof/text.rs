//! Exact Text/NULL checks with native declared storage qualification.
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker, header::check,
    table::mutation::slot::storage,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) fn validate(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !matches!(value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge(Resource::Steps, width(value))
        .map_err(InspectError::from)?;
    storage::validate(field, value)
}

pub(super) fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let work = width(left)
        .checked_add(width(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

fn width(value: &Value) -> u64 {
    match value {
        Value::Text(value) => value.len() as u64,
        _ => size_of::<Value>() as u64,
    }
}
