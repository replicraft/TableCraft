//! Eager selection can retain an Error data value without raising it.
use super::super::condition as scalar_condition;
use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

pub(super) fn condition(value: &ExpressionValue) -> Result<bool, EvalError> {
    match value {
        ExpressionValue::Value(value) => scalar_condition(value),
        ExpressionValue::Empty => Ok(false),
        ExpressionValue::Missing | ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
    }
}

/// Move one already evaluated branch without interpreting its payload.
/// Empty conditions are false without becoming stored NULL.
pub fn iif<T>(
    condition_value: &ExpressionValue,
    truepart: T,
    falsepart: T,
) -> Result<T, EvalError> {
    Ok(if condition(condition_value)? {
        truepart
    } else {
        falsepart
    })
}

/// Query NULL replacement; a Variant Error is distinct from SQL NULL.
pub fn nz(value: ExpressionValue, replacement: Option<ExpressionValue>) -> ExpressionValue {
    if matches!(value, ExpressionValue::Value(Value::Null)) {
        replacement.unwrap_or_else(|| ExpressionValue::Value(Value::Text(String::new())))
    } else {
        value
    }
}
