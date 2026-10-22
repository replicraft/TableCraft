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

/// Select already evaluated branches; Empty is false without becoming stored NULL.
pub fn iif(
    condition_value: &ExpressionValue,
    truepart: ExpressionValue,
    falsepart: ExpressionValue,
) -> Result<ExpressionValue, EvalError> {
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
