//! Date recognition keeps host failures distinct from invalid date values.
use crate::{
    EvalError,
    functions::{
        dates::parsing::{DateConversion, DateParseContext},
        text::{checked_buffer::CheckedBuffer, output::NumericBuffer},
    },
};
use tablecraft_core::values::Value;

pub(super) fn evaluate(
    value: &Value,
    mut context: DateParseContext,
    buffer: &mut impl NumericBuffer,
) -> Result<bool, EvalError> {
    if matches!(
        value,
        Value::Null | Value::Boolean(_) | Value::Binary(_) | Value::Guid(_)
    ) {
        return Ok(false);
    }
    // Recognition uses calendar/time spellings, not CDate's numeric Text fallback.
    context.number_format = None;
    let mut checked = CheckedBuffer {
        inner: buffer,
        failure: None,
    };
    let result = DateConversion::CDate.apply(value, context, &mut checked);
    if let Some(error) = checked.failure {
        return Err(error);
    }
    match result {
        Ok(Value::DateTime(_)) => Ok(true),
        Err(EvalError::TypeMismatch | EvalError::Overflow | EvalError::NonFinite) => Ok(false),
        Err(error) => Err(error),
        Ok(_) => Err(EvalError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests;
