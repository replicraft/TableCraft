//! Named Gregorian date/time formatting, independent of host locale and clocks.
mod render;
use super::{
    TextContext,
    output::{NumericBuffer, TextOutput},
};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::{
            parsing::{DateConversion, DateParseContext, DateTextFormat},
            scalar_calendar,
        },
    },
};
use jiff::{
    RoundMode, Unit,
    civil::{DateTime, DateTimeRound},
};
use tablecraft_core::values::Value;

/// The five numeric FormatDateTime selectors; values follow Access's named constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedDateFormat {
    /// Short date and/or long time, depending on the stored components.
    General,
    /// The selected profile's long date.
    LongDate,
    /// The selected profile's short date, including the epoch date.
    ShortDate,
    /// The selected profile's time with seconds.
    LongTime,
    /// Intrinsic 24-hour hour/minute output.
    ShortTime,
}
impl NamedDateFormat {
    fn from_value(value: &Value) -> Result<Self, EvalError> {
        let Value::Int32(value) = Conversion::Long.apply(value)? else {
            return Err(EvalError::TypeMismatch);
        };
        match value {
            0 => Ok(Self::General),
            1 => Ok(Self::LongDate),
            2 => Ok(Self::ShortDate),
            3 => Ok(Self::LongTime),
            4 => Ok(Self::ShortTime),
            _ => Err(EvalError::Domain),
        }
    }
}
/// Borrowed input recipe; resolve through a charged scratch buffer before output.
#[derive(Debug, PartialEq, Eq)]
pub struct DateTimeFormat<'a> {
    value: &'a Value,
    format: DateTextFormat,
    mode: NamedDateFormat,
    // Store explicit policies individually to retain structural equality of recipes.
    year_window: crate::functions::dates::TwoDigitYearWindow,
    reference_year: Option<crate::functions::dates::parsing::ReferenceYear>,
    number_format: Option<crate::functions::conversions::text::NumberFormat>,
    input_format: Option<DateTextFormat>,
}
/// Validated civil output recipe, with no remaining input or scratch lifetime.
#[derive(Debug, PartialEq, Eq)]
pub struct DateDisplay {
    civil: DateTime,
    format: DateTextFormat,
    mode: NamedDateFormat,
}
pub(super) fn prepare<'a>(
    arguments: &'a [Value],
    context: TextContext<'_>,
) -> Result<DateTimeFormat<'a>, EvalError> {
    prepare_mode(arguments, context, None)
}

pub(crate) fn prepare_mode<'a>(
    arguments: &'a [Value],
    context: TextContext<'_>,
    normalized_mode: Option<&Value>,
) -> Result<DateTimeFormat<'a>, EvalError> {
    let mode = normalized_mode
        .or_else(|| arguments.get(1))
        .map(NamedDateFormat::from_value)
        .transpose()?
        .unwrap_or(NamedDateFormat::General);
    let value = &arguments[0];
    if matches!(value, Value::Null) {
        return Err(EvalError::InvalidNullConversion);
    }
    let format = match (context.date_format, mode) {
        (Some(format), _) => format,
        (None, NamedDateFormat::ShortTime) => DateTextFormat::Iso,
        _ => return Err(EvalError::DateFormatRequired),
    };
    Ok(DateTimeFormat {
        value,
        format,
        mode,
        year_window: context.year_window,
        reference_year: context.reference_year,
        number_format: context.number_format,
        input_format: context.date_format,
    })
}
impl DateTimeFormat<'_> {
    /// Resolve numeric/Text inputs through CDate; scratch storage is never output.
    /// Existing extended dates retain their components instead of narrowing to DATE.
    pub fn resolve(&self, scratch: &mut impl NumericBuffer) -> Result<DateDisplay, EvalError> {
        scratch.step()?;
        if !scratch.text().is_empty() {
            return Err(EvalError::Domain);
        }
        let civil = match self.value {
            Value::DateTime(_) | Value::DateTimeExtended(_) => scalar_calendar(self.value)?,
            value => {
                let converted = DateConversion::CDate.apply(
                    value,
                    DateParseContext {
                        date_format: self.input_format,
                        year_window: self.year_window,
                        reference_year: self.reference_year,
                        number_format: self.number_format,
                    },
                    scratch,
                )?;
                scalar_calendar(&converted)?
            }
        };
        Ok(DateDisplay {
            civil: rounded(civil)?,
            format: self.format,
            mode: self.mode,
        })
    }
}
impl DateDisplay {
    /// Emit only through the caller's bounded, cancellation-aware output.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        render::write(self.civil, self.format, self.mode, output)
    }
}
pub(super) fn write_civil(
    civil: DateTime,
    format: DateTextFormat,
    mode: NamedDateFormat,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    output.step()?;
    render::write(civil, format, mode, output)
}

pub(super) fn rounded(civil: DateTime) -> Result<DateTime, EvalError> {
    civil
        .round(
            DateTimeRound::new()
                .smallest(Unit::Second)
                .mode(RoundMode::HalfExpand),
        )
        .map_err(|_| EvalError::Overflow)
}
pub(super) fn write_general(
    civil: DateTime,
    format: DateTextFormat,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    render::write(rounded(civil)?, format, NamedDateFormat::General, output)
}
#[cfg(test)]
mod tests;
