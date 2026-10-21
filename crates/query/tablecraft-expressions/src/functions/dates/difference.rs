//! Signed Gregorian boundary counts, independently of elapsed calendar spans.
use super::{
    conversion,
    interval::Interval,
    weeks::{WeekContext, WeekSettings},
};
use crate::EvalError;
use jiff::civil::{DateTime, date};
use tablecraft_core::values::Value;

const MONTHS_PER_YEAR: i64 = 12;
const MONTHS_PER_QUARTER: i64 = 3;
const DAYS_PER_WEEK: i64 = 7;
const HOURS_PER_DAY: i64 = 24;
const MINUTES_PER_HOUR: i64 = 60;
const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_DAY: i64 = HOURS_PER_DAY * MINUTES_PER_HOUR * SECONDS_PER_MINUTE;
const DEFAULT_WEEK_OPTION: i32 = 1;

/// Count signed Access intervals between evaluated explicit dates, returning Int32.
/// Optional week settings preserve absence versus NULL. Sunday and Jan-1-week
/// defaults are explicit; zero (system policy) fails without a host policy. Week
/// labels do not alter the sequence of calendar-week boundaries. Nondefault first
/// days with `w` remain unsupported pending resolution of native anchor behavior.
/// Hosts must eagerly evaluate all supplied arguments in written order.
pub fn date_diff(
    interval: &Value,
    first: &Value,
    second: &Value,
    first_day: Option<&Value>,
    first_week: Option<&Value>,
) -> Result<Value, EvalError> {
    date_diff_with_context(
        interval,
        first,
        second,
        first_day,
        first_week,
        WeekContext::default(),
    )
}

/// Apply the same Gregorian profile with caller-issued system week settings.
/// Concrete SQL options override context; omitted options keep their documented defaults.
pub fn date_diff_with_context(
    interval: &Value,
    first: &Value,
    second: &Value,
    first_day: Option<&Value>,
    first_week: Option<&Value>,
    context: WeekContext,
) -> Result<Value, EvalError> {
    let Value::Text(interval) = interval else {
        return Err(EvalError::TypeMismatch);
    };
    let interval = Interval::resolve(interval).ok_or(EvalError::InvalidDateInterval)?;
    let settings = WeekSettings::parse(first_day, first_week, context)?;
    let first_day = settings.first_day;
    if matches!(interval, Interval::Weekday) && first_day != DEFAULT_WEEK_OPTION {
        return Err(EvalError::UnsupportedExpression);
    }
    if [first, second].into_iter().any(|value| {
        !matches!(
            value,
            Value::Null | Value::DateTime(_) | Value::DateTimeExtended(_)
        )
    }) {
        return Err(EvalError::TypeMismatch);
    }
    if matches!(first, Value::Null) || matches!(second, Value::Null) {
        return Ok(Value::Null);
    }
    let first = conversion::calendar(first)?;
    let second = conversion::calendar(second)?;
    let days_first = ordinal(first);
    let days_second = ordinal(second);
    let difference = match interval {
        Interval::Year => i64::from(second.year()) - i64::from(first.year()),
        Interval::Quarter => quarter(second) - quarter(first),
        Interval::Month => month(second) - month(first),
        Interval::Day | Interval::DayOfYear => days_second - days_first,
        Interval::Weekday => (days_second - days_first) / DAYS_PER_WEEK,
        Interval::Week => {
            let week = |civil: DateTime, day: i64| {
                let weekday = i64::from(civil.weekday().to_sunday_zero_offset());
                let offset = (weekday - i64::from(first_day - DEFAULT_WEEK_OPTION))
                    .rem_euclid(DAYS_PER_WEEK);
                // Any fixed epoch works: subtracting week starts removes it.
                day - offset
            };
            (week(second, days_second) - week(first, days_first)) / DAYS_PER_WEEK
        }
        Interval::Hour => hour(second, days_second) - hour(first, days_first),
        Interval::Minute => minute(second, days_second) - minute(first, days_first),
        Interval::Second => second_ordinal(second, days_second) - second_ordinal(first, days_first),
    };
    difference
        .try_into()
        .map(Value::Int32)
        .map_err(|_| EvalError::Overflow)
}
fn ordinal(value: DateTime) -> i64 {
    value.date().duration_since(date(1970, 1, 1)).as_secs() / SECONDS_PER_DAY
}
fn month(value: DateTime) -> i64 {
    i64::from(value.year()) * MONTHS_PER_YEAR + i64::from(value.month() - 1)
}
fn quarter(value: DateTime) -> i64 {
    month(value).div_euclid(MONTHS_PER_QUARTER)
}
fn hour(value: DateTime, day: i64) -> i64 {
    day * HOURS_PER_DAY + i64::from(value.hour())
}
fn minute(value: DateTime, day: i64) -> i64 {
    hour(value, day) * MINUTES_PER_HOUR + i64::from(value.minute())
}
fn second_ordinal(value: DateTime, day: i64) -> i64 {
    minute(value, day) * SECONDS_PER_MINUTE + i64::from(value.second())
}
#[cfg(test)]
mod tests;
