//! Paid representational preconditions for the admitted long-value carriers.
use super::super::EditError;
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const COMPARE_BYTES: usize = 256;

pub(super) fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let (left, right) = match (left, right) {
        (Value::Null, Value::Null) => return Ok(true),
        (Value::Text(left), Value::Text(right)) => (left.as_bytes(), right.as_bytes()),
        (Value::Binary(left), Value::Binary(right)) => (left.as_slice(), right.as_slice()),
        _ => return Ok(false),
    };
    if left.len() != right.len() {
        return Ok(false);
    }
    budget
        .charge(Resource::Steps, left.len() as u64)
        .map_err(InspectError::from)?;
    for (left, right) in left.chunks(COMPARE_BYTES).zip(right.chunks(COMPARE_BYTES)) {
        check(budget, cancelled)?;
        if left != right {
            return Ok(false);
        }
    }

    Ok(true)
}
