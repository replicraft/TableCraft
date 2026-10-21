//! IsNumeric's expected lexical failures are distinct from operational failures.
use crate::functions::text::checked_buffer::CheckedBuffer;
use crate::{
    EvalError,
    functions::{
        conversions::{Conversion, text::NumberFormat},
        text::output::NumericBuffer,
    },
};
use tablecraft_core::values::Value;

pub(super) fn evaluate(
    value: &Value,
    format: Option<NumberFormat>,
    buffer: &mut impl NumericBuffer,
) -> Result<bool, EvalError> {
    match value {
        Value::Boolean(_)
        | Value::Byte(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::Single(_)
        | Value::Double(_)
        | Value::Currency(_)
        | Value::Decimal(_) => Ok(true),
        Value::Text(text) => {
            let format = format.ok_or(EvalError::NumberFormatRequired)?;
            let mut checked = CheckedBuffer {
                inner: buffer,
                failure: None,
            };
            let result = Conversion::Double.apply_text(text, format, &mut checked);
            if let Some(error) = checked.failure {
                return Err(error);
            }
            match result {
                Ok(Value::Double(_)) => Ok(true),
                Err(EvalError::TypeMismatch | EvalError::Overflow) => Ok(false),
                Err(error) => Err(error),
                Ok(_) => Err(EvalError::TypeMismatch),
            }
        }
        Value::Null
        | Value::DateTime(_)
        | Value::DateTimeExtended(_)
        | Value::Binary(_)
        | Value::Guid(_) => Ok(false),
    }
}
#[cfg(test)]
mod tests;
