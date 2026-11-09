//! CStr borrows Text/Boolean spellings and emits numeric/date recipes under policies.
mod date;
mod errors;
mod numeric;
use super::{TextContext, TextResult, numeric_format};
use crate::EvalError;
pub use date::{DateText, DateTextFormat};
pub use errors::write_standard_error;
pub(super) use numeric::NumericOutput;
use tablecraft_core::values::Value;

pub(super) fn prepare<'a>(
    value: &'a Value,
    context: TextContext<'_>,
) -> Result<TextResult<'a>, EvalError> {
    match value {
        Value::Null => Err(EvalError::InvalidNullConversion),
        Value::Text(text) => Ok(TextResult::Text(text)),
        Value::Boolean(value) => Ok(TextResult::Text(if *value { "True" } else { "False" })),
        Value::DateTime(_) | Value::DateTimeExtended(_) => {
            Ok(TextResult::DateFormat(DateText::new(
                value,
                context.date_format.ok_or(EvalError::DateFormatRequired)?,
            )))
        }
        Value::Binary(_) | Value::Guid(_) => Err(EvalError::TypeMismatch),
        _ => numeric_format::prepare_cstr(
            value,
            context
                .number_format
                .ok_or(EvalError::NumberFormatRequired)?,
        ),
    }
}
#[cfg(test)]
mod tests;
