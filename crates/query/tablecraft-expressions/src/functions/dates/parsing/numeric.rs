//! Numeric date coercion and explicit whole-Text fallback with host-error provenance.
use crate::{
    EvalError,
    functions::{
        conversions::{Conversion, text::NumberFormat},
        text::{checked_buffer::CheckedBuffer, output::NumericBuffer},
    },
};
use tablecraft_core::values::{LegacyDateTime, Value};

pub(super) fn value(value: &Value) -> Result<LegacyDateTime, EvalError> {
    let Value::Double(number) = Conversion::Double.apply(value)? else {
        return Err(EvalError::TypeMismatch);
    };
    LegacyDateTime::from_ole_days(number.to_float()).map_err(|_| EvalError::Overflow)
}
pub(super) fn text(
    value: &str,
    format: NumberFormat,
    buffer: &mut impl NumericBuffer,
) -> Result<LegacyDateTime, EvalError> {
    let mut checked = CheckedBuffer {
        inner: buffer,
        failure: None,
    };
    let result = Conversion::Double.apply_text(value, format, &mut checked);
    if let Some(error) = checked.failure {
        return Err(error);
    }
    let Value::Double(number) = result.map_err(|error| {
        if error == EvalError::Overflow {
            EvalError::TypeMismatch
        } else {
            error
        }
    })?
    else {
        return Err(EvalError::TypeMismatch);
    };
    LegacyDateTime::from_ole_days(number.to_float()).map_err(|_| EvalError::TypeMismatch)
}
