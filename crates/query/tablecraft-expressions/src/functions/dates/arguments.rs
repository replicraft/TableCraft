//! Checked scalar controls shared by date interval and week functions.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

pub(crate) fn long(value: &Value) -> Result<i32, EvalError> {
    // Date control arguments retain their existing materialized NULL-option error.
    // Format performs its separate InvalidNullConversion preflight first.
    if matches!(value, Value::Null) {
        return Err(EvalError::TypeMismatch);
    }
    match Conversion::Long.apply(value)? {
        Value::Int32(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}
