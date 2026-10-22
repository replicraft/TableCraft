//! Choose index coercion keeps expression-only states outside stored values.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{ExpressionValue, Value};

pub(super) fn select(value: &ExpressionValue, choices: usize) -> Result<Option<usize>, EvalError> {
    let value = match value {
        ExpressionValue::Value(Value::Null) | ExpressionValue::Empty => return Ok(None),
        ExpressionValue::Value(value) => value,
        ExpressionValue::Missing | ExpressionValue::Error(_) => {
            return Err(EvalError::TypeMismatch);
        }
    };
    match Conversion::LongLong.apply(value) {
        Ok(Value::Int64(value)) => Ok(usize::try_from(value)
            .ok()
            .filter(|index| *index >= 1 && *index <= choices)),
        Err(EvalError::Overflow) => Ok(None),
        Err(error) => Err(error),
        _ => Err(EvalError::TypeMismatch),
    }
}
