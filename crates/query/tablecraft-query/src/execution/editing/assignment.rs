//! Shared explicit stored-assignment laws for actual UPDATE and INSERT planners.
use crate::QueryError;
use tablecraft_core::values::Value;
use tablecraft_db::UpdateProfile;
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, ScalarType};

pub(super) fn update_value(
    value: Value,
    target: &Column,
    profile: UpdateProfile,
) -> Result<Value, QueryError> {
    match profile {
        UpdateProfile::ExistingInt32 => integral_value(value),
        UpdateProfile::StoredScalar => stored_value(value, target),
        UpdateProfile::Unavailable => Err(QueryError::Unsupported),
    }
}

/// Nullable NULL, exact non-Int32 stored carriers, and checked integral Int32.
/// This is an assignment rule, not permission to change native field metadata.
pub(in crate::execution) fn stored_value(
    value: Value,
    target: &Column,
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

    if ScalarType::of(&value) != Some(target.data_type())
        || target.type_descriptor().validate_value(&value).is_err()
    {
        return Err(QueryError::Unsupported);
    }

    Ok(value)
}

fn integral_value(value: Value) -> Result<Value, QueryError> {
    let integer = match value {
        Value::Byte(value) => i64::from(value),
        Value::Int16(value) => i64::from(value),
        Value::Int32(value) => i64::from(value),
        Value::Int64(value) => value,
        _ => return Err(QueryError::Unsupported),
    };

    i32::try_from(integer)
        .map(Value::Int32)
        .map_err(|_| EvalError::Overflow.into())
}
