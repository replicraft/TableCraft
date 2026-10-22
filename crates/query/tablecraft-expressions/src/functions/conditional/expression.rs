//! Eager selection can retain an Error data value without raising it.
use super::super::condition;
use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

pub(super) fn scalar(value: &ExpressionValue) -> Result<&Value, EvalError> {
    match value {
        ExpressionValue::Value(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}

/// Select already evaluated branches; the condition still requires an ordinary scalar.
pub fn iif(
    condition_value: &ExpressionValue,
    truepart: ExpressionValue,
    falsepart: ExpressionValue,
) -> Result<ExpressionValue, EvalError> {
    Ok(if condition(scalar(condition_value)?)? {
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
