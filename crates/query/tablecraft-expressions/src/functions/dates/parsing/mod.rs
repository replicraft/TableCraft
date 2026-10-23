//! CDate/CVDate/DateValue/TimeValue with explicit Gregorian text policy and no clock lookup.
mod date;
mod numeric;
mod policy;
mod text;
mod time;
use super::conversion;
use crate::{EvalError, functions::text::output::NumericBuffer};
use jiff::civil::{Date, Time};
pub use policy::{DateParseContext, DateTextFormat, ReferenceYear};
use tablecraft_core::values::{LegacyDateTime, Value};

const AUTOMATION_EPOCH: Date = Date::constant(1899, 12, 30);

/// Closed, eager date conversion/extraction calls over materialized values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateConversion {
    /// Convert a supported scalar/date/time spelling into legacy DATE.
    CDate,
    /// Same DATE conversion, retaining the declared Variant-returning identity.
    CVDate,
    /// Return the date portion with midnight time; NULL propagates.
    DateValue,
    /// Return the time portion on the Automation epoch date; NULL propagates.
    TimeValue,
}
impl DateConversion {
    /// Resolve a supported unqualified ASCII-insensitive function name.
    /// CDate/CVDate share conversion behavior but retain distinct function identity.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("CDate", Self::CDate),
            ("CVDate", Self::CVDate),
            ("DateValue", Self::DateValue),
            ("TimeValue", Self::TimeValue),
        ]
        .into_iter()
        .find_map(|(spelling, function)| name.eq_ignore_ascii_case(spelling).then_some(function))
    }
    /// Apply one already evaluated value under explicit date/numeric policies.
    /// The initially empty buffer meters work and any numeric Text normalization.
    pub fn apply(
        self,
        value: &Value,
        context: DateParseContext,
        buffer: &mut impl NumericBuffer,
    ) -> Result<Value, EvalError> {
        buffer.step()?;
        if !buffer.text().is_empty() {
            return Err(EvalError::Domain);
        }
        if matches!(value, Value::Null) {
            return if matches!(self, Self::CDate | Self::CVDate) {
                Err(EvalError::InvalidNullConversion)
            } else {
                Ok(Value::Null)
            };
        }
        if let Value::DateTimeExtended(value) = value {
            let civil = conversion::extended_calendar(*value)?;
            let civil = project(self, civil);
            return legacy(civil).map(Value::DateTime);
        }
        let result = match value {
            Value::DateTime(value) => *value,
            Value::Text(value) => {
                for _ in value.chars() {
                    buffer.step()?;
                }
                context.date_format.ok_or(EvalError::DateFormatRequired)?;
                match text::parse(value, context) {
                    Ok(civil) => return legacy(project(self, civil)).map(Value::DateTime),
                    Err(EvalError::TypeMismatch)
                        if matches!(self, Self::CDate | Self::CVDate)
                            && context.number_format.is_some() =>
                    {
                        numeric::text(
                            value,
                            context
                                .number_format
                                .ok_or(EvalError::NumberFormatRequired)?,
                            buffer,
                        )?
                    }
                    Err(error) => return Err(error),
                }
            }
            _ => numeric::value(value)?,
        };
        Ok(Value::DateTime(match self {
            Self::CDate | Self::CVDate => result,
            Self::DateValue => LegacyDateTime::from_ole_days(f64::from(result.day_offset()))
                .map_err(|_| EvalError::Overflow)?,
            Self::TimeValue => LegacyDateTime::from_ole_days(result.fraction_of_day())
                .map_err(|_| EvalError::Overflow)?,
        }))
    }
}
fn project(function: DateConversion, civil: jiff::civil::DateTime) -> jiff::civil::DateTime {
    match function {
        DateConversion::CDate | DateConversion::CVDate => civil,
        DateConversion::DateValue => civil.date().to_datetime(Time::MIN),
        DateConversion::TimeValue => AUTOMATION_EPOCH.to_datetime(civil.time()),
    }
}
fn legacy(value: jiff::civil::DateTime) -> Result<LegacyDateTime, EvalError> {
    LegacyDateTime::from_calendar(value).map_err(|_| EvalError::Overflow)
}
#[cfg(test)]
mod tests;
