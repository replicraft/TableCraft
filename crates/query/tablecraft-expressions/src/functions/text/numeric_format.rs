//! Numeric text recipes with explicit policies and charged host output.
pub(super) mod floating;
pub(super) mod magnitude;
mod radix;
use super::format_buffer::FormatBuffer as Literal;
use super::{TextFunction, TextResult, output::TextOutput};
use crate::EvalError;
use crate::functions::conversions::text::NumberFormat;
use std::fmt::Write;
use tablecraft_core::values::Value;

/// Borrowed numeric-to-Text operation. Hosts discard partial output on any error.
#[derive(Debug, PartialEq, Eq)]
pub struct NumericFormat<'a> {
    function: TextFunction,
    value: &'a Value,
    number_format: Option<NumberFormat>,
}
pub(super) fn prepare(function: TextFunction, value: &Value) -> Result<TextResult<'_>, EvalError> {
    if matches!(value, Value::Null) {
        return Ok(TextResult::Null);
    }
    let legacy_radix = matches!(function, TextFunction::Hex | TextFunction::Oct)
        && matches!(value, Value::DateTime(_));
    if !legacy_radix
        && !matches!(
            value,
            Value::Boolean(_)
                | Value::Byte(_)
                | Value::Int16(_)
                | Value::Int32(_)
                | Value::Int64(_)
                | Value::Single(_)
                | Value::Double(_)
                | Value::Currency(_)
                | Value::Decimal(_)
        )
    {
        return Err(EvalError::TypeMismatch);
    }
    Ok(TextResult::NumericFormat(NumericFormat {
        function,
        value,
        number_format: None,
    }))
}
pub(super) fn prepare_cstr(
    value: &Value,
    format: NumberFormat,
) -> Result<TextResult<'_>, EvalError> {
    let mut recipe = if matches!(value, Value::DateTime(_)) {
        // Format's General Number explicitly selects a serial display. CStr
        // and Str retain their separate date/profile and argument contracts.
        NumericFormat {
            function: TextFunction::Str,
            value,
            number_format: None,
        }
    } else {
        let TextResult::NumericFormat(recipe) = prepare(TextFunction::Str, value)? else {
            return Err(EvalError::TypeMismatch);
        };
        recipe
    };
    recipe.number_format = Some(format);
    Ok(TextResult::NumericFormat(recipe))
}
impl NumericFormat<'_> {
    /// Render using bounded stack scratch, charging host output before allocation.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        if let Some(format) = self.number_format {
            return self.write_raw(&mut super::cstr::NumericOutput::new(output, format));
        }
        self.write_raw(output)
    }
    fn write_raw(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        if self.function != TextFunction::Str {
            return radix::write(self.function, self.value, output);
        }
        let mut text = Literal::new();
        match self.value {
            Value::Boolean(value) => write!(text, "{}", if *value { -1 } else { 0 }),
            Value::Byte(value) => write!(text, "{value}"),
            Value::Int16(value) => write!(text, "{value}"),
            Value::Int32(value) => write!(text, "{value}"),
            Value::Int64(value) => write!(text, "{value}"),
            Value::Currency(value) => write!(text, "{value}"),
            Value::Decimal(value) => write!(text, "{value}"),
            Value::Single(value) => {
                return floating::write(
                    f64::from(value.to_float()),
                    floating::SINGLE_DIGITS,
                    output,
                );
            }
            Value::Double(value) => {
                return floating::write(value.to_float(), floating::DOUBLE_DIGITS, output);
            }
            Value::DateTime(value) if self.number_format.is_some() => {
                return floating::write(value.ole_days(), floating::DOUBLE_DIGITS, output);
            }
            _ => return Err(EvalError::TypeMismatch),
        }
        .map_err(|_| EvalError::TextLimit)?;
        let mut text = text.text()?;
        if text.contains('.') {
            text = text.trim_end_matches('0').trim_end_matches('.');
        }
        if !text.starts_with('-') {
            output.append(" ")?;
        }
        output.append(text)
    }
}
#[cfg(test)]
mod tests;
