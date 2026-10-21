//! Explicit-profile FormatNumber/Percent/Currency, with private conversion scratch.
mod magnitude;
mod options;
mod render;
use super::{
    TextContext, TextFunction,
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
    let value = &arguments[0];
    if matches!(value, Value::Null) {
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
    let options = options::Options::parse(kind, arguments, format)?;
    Ok(StyledNumber {
        value,
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
        let value = if let Value::Text(text) = self.value {
            parsed = Conversion::Double.apply_text(text, self.format, scratch)?;
            &parsed
        } else {
            self.value
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
