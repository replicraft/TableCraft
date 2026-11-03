//! Shared explicit stored-assignment laws for actual UPDATE and INSERT planners.
use crate::QueryError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_db::UpdateProfile;
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, ScalarType};

const CURRENCY_UNITS_PER_INTEGER: i64 = 10_i64.pow(Currency::SCALE as u32);

#[cfg(test)]
mod tests;

pub(super) fn update_value(
    value: Value,
    target: &Column,
    profile: UpdateProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, QueryError> {
    match profile {
        UpdateProfile::ExistingInt32 => integral_value(value),
        UpdateProfile::StoredScalar => stored_value(value, target, budget, cancelled),
        UpdateProfile::Unavailable => Err(QueryError::Unsupported),
    }
}

/// Nullable NULL, checked integral assignments, and exact other stored carriers.
/// This is an assignment rule, not permission to change native field metadata.
pub(in crate::execution) fn stored_value(
    value: Value,
    target: &Column,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, QueryError> {
    if matches!(value, Value::Null) {
        return if target.nullable() {
            Ok(value)
        } else {
            Err(QueryError::Unsupported)
        };
    }

    if target.data_type() == ScalarType::Int32 {
        return integral_value(value);
    }

    let value = match (target.data_type(), &value) {
        (ScalarType::Int64, Value::Byte(_) | Value::Int16(_) | Value::Int32(_))
        | (
            ScalarType::Currency,
            Value::Byte(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_),
        ) => {
            // Only this newly admitted fixed-size conversion consumes extra work.
            // Existing exact carriers and Int32 retain their original path.
            if cancelled() {
                return Err(QueryError::Cancelled);
            }
            budget.charge(Resource::Steps, 1)?;

            let integer = integer(&value).ok_or(QueryError::Unsupported)?;
            if target.data_type() == ScalarType::Int64 {
                Value::Int64(integer)
            } else {
                let scaled = integer
                    .checked_mul(CURRENCY_UNITS_PER_INTEGER)
                    .ok_or(EvalError::Overflow)?;
                Value::Currency(Currency::from_scaled(scaled))
            }
        }
        _ => value,
    };

    if ScalarType::of(&value) != Some(target.data_type())
        || target.type_descriptor().validate_value(&value).is_err()
    {
        return Err(QueryError::Unsupported);
    }

    Ok(value)
}

fn integral_value(value: Value) -> Result<Value, QueryError> {
    let integer = integer(&value).ok_or(QueryError::Unsupported)?;

    i32::try_from(integer)
        .map(Value::Int32)
        .map_err(|_| EvalError::Overflow.into())
}

fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Byte(value) => Some(i64::from(*value)),
        Value::Int16(value) => Some(i64::from(*value)),
        Value::Int32(value) => Some(i64::from(*value)),
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}
