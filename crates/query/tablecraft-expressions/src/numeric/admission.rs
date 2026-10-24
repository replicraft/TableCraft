//! Arithmetic carrier admission precedes NULL results; no values are coerced here.
use crate::EvalError;
use tablecraft_core::values::Value;

pub(crate) fn strict(value: &Value) -> Result<(), EvalError> {
    match value {
        Value::Null
        | Value::Boolean(_)
        | Value::Byte(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::Currency(_)
        | Value::Decimal(_) => Ok(()),
        Value::Single(value) if value.to_float().is_finite() => Ok(()),
        Value::Double(value) if value.to_float().is_finite() => Ok(()),
        Value::Single(_) | Value::Double(_) => Err(EvalError::NonFinite),
        _ => Err(EvalError::TypeMismatch),
    }
}

pub(crate) fn materialized(value: &Value) -> Result<(), EvalError> {
    if matches!(value, Value::DateTime(_)) {
        return Ok(());
    }

    strict(value)
}

pub(crate) fn contextual(value: &Value) -> Result<(), EvalError> {
    if matches!(value, Value::Text(_)) {
        return Ok(());
    }

    materialized(value)
}

#[cfg(test)]
mod tests;
