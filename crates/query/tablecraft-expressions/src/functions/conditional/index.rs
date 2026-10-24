//! Choose index coercion keeps expression-only states outside stored values.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{ExpressionValue, Value};

pub(super) fn select(value: &ExpressionValue, choices: usize) -> Result<Option<usize>, EvalError> {
    let value = match value {
        ExpressionValue::Value(Value::Null) | ExpressionValue::Empty => return Ok(None),
        ExpressionValue::Value(value) => value,
        ExpressionValue::Missing | ExpressionValue::Error(_) | ExpressionValue::StringBytes(_) => {
            return Err(EvalError::TypeMismatch);
        }
    };
    let single = Conversion::Single.apply(value)?;
    match Conversion::LongLong.apply(&single) {
        Ok(Value::Int64(value)) => {
            let index = usize::try_from(value)
                .ok()
                .filter(|index| *index >= 1 && *index <= choices);
            if index.is_some() {
                // The formal selector converts Single to Integer before use.
                // Out-of-list values retain the documented NULL outcome.
                Conversion::Integer.apply(&single)?;
            }
            Ok(index)
        }
        Err(EvalError::Overflow) => Ok(None),
        Err(error) => Err(error),
        _ => Err(EvalError::TypeMismatch),
    }
}
