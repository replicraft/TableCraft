//! Bounded DateAdd over explicit Gregorian date carriers.
use super::{conversion, interval::Interval};
use crate::{
    EvalError,
    numeric::{self, IntegerType},
};
use jiff::Span;
use tablecraft_core::values::{LegacyDateTime, Value};

const MONTHS_PER_QUARTER: i64 = 3;

/// Add one of the ten Access interval spellings to an already evaluated date.
/// Counts use the shared signed Long conversion (nearest integer, ties even).
/// NULL date stays NULL; interval/count NULL have no implicit conversion. Legacy
/// dates use the whole-second function profile; Extended dates retain 100 ns.
/// Calendar overflow fails, month/year additions constrain to a valid month end.
/// Hosts must evaluate all three operands in written order before calling.
pub fn date_add(interval: &Value, number: &Value, date: &Value) -> Result<Value, EvalError> {
    let Value::Text(interval) = interval else {
        return Err(EvalError::TypeMismatch);
    };
    let interval = Interval::resolve(interval).ok_or(EvalError::InvalidDateInterval)?;
    let Value::Int32(number) = numeric::coerce_integer(number, IntegerType::Int32)? else {
        return Err(EvalError::TypeMismatch);
    };
    // Validate both scalar control arguments before propagating a NULL date.
    if matches!(date, Value::Null) {
        return Ok(Value::Null);
    }
    let civil = match date {
        Value::DateTime(value) => conversion::legacy_calendar(*value)?,
        Value::DateTimeExtended(value) => conversion::extended_calendar(*value)?,
        _ => return Err(EvalError::TypeMismatch),
    };
    let span = match interval {
        Interval::Year => Span::new().try_years(number),
        Interval::Quarter => Span::new().try_months(i64::from(number) * MONTHS_PER_QUARTER),
        Interval::Month => Span::new().try_months(number),
        Interval::Day | Interval::Weekday => Span::new().try_days(number),
        Interval::Week => Span::new().try_weeks(number),
        Interval::Hour => Span::new().try_hours(number),
        Interval::Minute => Span::new().try_minutes(number),
        Interval::Second => Span::new().try_seconds(number),
    }
    .map_err(|_| EvalError::Overflow)?;
    let result = civil.checked_add(span).map_err(|_| EvalError::Overflow)?;
    match date {
        Value::DateTime(_) => LegacyDateTime::from_calendar(result)
            .map(Value::DateTime)
            .map_err(|_| EvalError::Overflow),
        Value::DateTimeExtended(_) => {
            conversion::extended_value(result).map(Value::DateTimeExtended)
        }
        _ => Err(EvalError::TypeMismatch),
    }
}
#[cfg(test)]
mod tests;
