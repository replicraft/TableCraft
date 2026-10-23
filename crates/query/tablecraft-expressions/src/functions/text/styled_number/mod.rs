//! Explicit-profile FormatNumber/Percent/Currency, with private conversion scratch.
mod options;
mod render;
use super::{
    TextContext, TextFunction,
    numeric_format::magnitude,
    output::{NumericBuffer, TextOutput},
};
use crate::{
    EvalError,
    functions::conversions::{Conversion, text::NumberFormat},
};
use tablecraft_core::values::Value;

const PERCENT_DECIMAL_SHIFT: u32 = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Number,
    Percent,
    Currency,
}
/// Borrowed numeric formatting recipe; resolve through private charged scratch.
#[derive(Debug, PartialEq, Eq)]
pub struct StyledNumber<'a> {
    value: &'a Value,
    kind: Kind,
    options: options::Options,
    format: NumberFormat,
}
/// Resolved finite numeric display, independent of input/scratch lifetime.
#[derive(Debug, PartialEq, Eq)]
pub struct NumberDisplay {
    magnitude: magnitude::Magnitude,
    kind: Kind,
    options: options::Options,
    format: NumberFormat,
}
pub(super) fn prepare<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    context: TextContext<'_>,
) -> Result<StyledNumber<'a>, EvalError> {
    prepare_with(
        function,
        arguments,
        context,
        |value| match Conversion::Long.apply(value)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        },
    )
}

pub(crate) fn prepare_with<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    context: TextContext<'_>,
    mut convert: impl FnMut(&Value) -> Result<i32, EvalError>,
) -> Result<StyledNumber<'a>, EvalError> {
    prepare_inputs(
        function,
        matches!(&arguments[0], Value::Null),
        arguments.len(),
        context,
        || Ok(&arguments[0]),
        |index| convert(&arguments[index]),
    )
}

pub(crate) fn prepare_inputs<'a>(
    function: TextFunction,
    null_source: bool,
    count: usize,
    context: TextContext<'_>,
    value: impl FnOnce() -> Result<&'a Value, EvalError>,
    mut convert: impl FnMut(usize) -> Result<i32, EvalError>,
) -> Result<StyledNumber<'a>, EvalError> {
    if null_source {
        return Err(EvalError::InvalidNullConversion);
    }
    let kind = match function {
        TextFunction::FormatNumber => Kind::Number,
        TextFunction::FormatPercent => Kind::Percent,
        TextFunction::FormatCurrency => Kind::Currency,
        _ => return Err(EvalError::UnsupportedExpression),
    };
    let format = context
        .number_format
        .ok_or(EvalError::NumberFormatRequired)?;
    let options = options::Options::parse(kind, count, format, &mut convert)?;
    Ok(StyledNumber {
        value: value()?,
        kind,
        options,
        format,
    })
}
impl StyledNumber<'_> {
    /// Parse Text and round a bounded magnitude; caller discards normalization storage.
    pub fn resolve(&self, scratch: &mut impl NumericBuffer) -> Result<NumberDisplay, EvalError> {
        scratch.step()?;
        if !scratch.text().is_empty() {
            return Err(EvalError::Domain);
        }
        let parsed;
        let value = match self.value {
            Value::Text(text) => {
                parsed = Conversion::Double.apply_text(text, self.format, scratch)?;
                &parsed
            }
            Value::DateTime(_) => {
                // Numeric formatting consumes the stored serial, not civil time.
                parsed = Conversion::Double.apply(self.value)?;
                &parsed
            }
            value => value,
        };
        let shift = if self.kind == Kind::Percent {
            PERCENT_DECIMAL_SHIFT
        } else {
            0
        };
        let magnitude = magnitude::prepare(value, self.options.places, shift, scratch)?;
        Ok(NumberDisplay {
            magnitude,
            kind: self.kind,
            options: self.options,
            format: self.format,
        })
    }
}
impl NumberDisplay {
    /// Stream all digits/separators through bounded, fallible host output.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        render::write(self, output)
    }
}
#[cfg(test)]
mod tests;
