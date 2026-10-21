//! Gregorian date functions and DATE serial operators without clocks or text coercion.
mod add;
mod arithmetic;
mod constructors;
mod conversion;
mod difference;
mod first_week;
mod interval;
pub mod parsing;
mod part;
mod week_start;
mod weekday;
mod weeks;

use crate::EvalError;
use tablecraft_core::values::Value;

pub use add::date_add;
pub use arithmetic::date_arithmetic;
pub use constructors::{DateConstructor, TwoDigitYearWindow};
pub(crate) use conversion::calendar as scalar_calendar;
pub use difference::{date_diff, date_diff_with_context};
pub use first_week::FirstWeekOfYear;
pub use part::{date_part, date_part_with_context};
pub use week_start::FirstDayOfWeek;
pub use weekday::weekday;
pub use weeks::WeekContext;

/// Closed single-argument date component functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateFunction {
    /// Gregorian year.
    Year,
    /// Month in 1..=12.
    Month,
    /// Day of month in 1..=31.
    Day,
    /// Hour in 0..=23.
    Hour,
    /// Minute in 0..=59.
    Minute,
    /// Second in 0..=59.
    Second,
}
impl DateFunction {
    /// Resolve only ASCII-insensitive unqualified names.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Year", Self::Year),
            ("Month", Self::Month),
            ("Day", Self::Day),
            ("Hour", Self::Hour),
            ("Minute", Self::Minute),
            ("Second", Self::Second),
        ]
        .into_iter()
        .find_map(|(spelling, operation)| name.eq_ignore_ascii_case(spelling).then_some(operation))
    }
    /// Extract an Int16 component from DateTime/ExtendedDateTime or return NULL.
    /// Legacy DATE uses the existing nanosecond calendar conversion followed by
    /// nearest-second rounding (half expands); day carry is checked by Jiff.
    /// Extended values use exact stored components, without fraction rounding or
    /// legacy conversion. No text/numeric coercion or alternate calendar is inferred.
    pub fn apply(self, value: &Value) -> Result<Value, EvalError> {
        let (year, month, day, hour, minute, second) = match value {
            Value::Null => return Ok(Value::Null),
            Value::DateTime(value) => {
                let civil = conversion::legacy_calendar(*value)?;
                (
                    civil.year(),
                    i16::from(civil.month()),
                    i16::from(civil.day()),
                    i16::from(civil.hour()),
                    i16::from(civil.minute()),
                    i16::from(civil.second()),
                )
            }
            Value::DateTimeExtended(value) => {
                let (year, month, day) = value.date();
                let (hour, minute, second, _) = value.time();
                (
                    year as i16,
                    i16::from(month),
                    i16::from(day),
                    i16::from(hour),
                    i16::from(minute),
                    i16::from(second),
                )
            }
            _ => return Err(EvalError::TypeMismatch),
        };
        Ok(Value::Int16(match self {
            Self::Year => year,
            Self::Month => month,
            Self::Day => day,
            Self::Hour => hour,
            Self::Minute => minute,
            Self::Second => second,
        }))
    }
}
#[cfg(test)]
mod tests;
