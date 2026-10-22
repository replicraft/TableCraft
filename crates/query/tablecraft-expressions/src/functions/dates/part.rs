//! Gregorian DatePart over explicit dates and explicit week settings.
use super::{
    conversion,
    interval::Interval,
    weeks::{WeekContext, WeekSettings},
};
use crate::EvalError;
use tablecraft_core::values::Value;

const MONTHS_PER_QUARTER: i8 = 3;

/// Extract an Int16 Access date part with optional week settings kept distinct
/// from NULL. Legacy dates use the shared whole-second function profile; Extended
/// values retain exact components. System settings require an explicit host policy.
/// Week numbering preserves Microsoft's documented final-Monday anomaly.
/// Hosts must evaluate all provided arguments in written order before calling.
pub fn date_part(
    interval: &Value,
    date: &Value,
    first_day: Option<&Value>,
    first_week: Option<&Value>,
) -> Result<Value, EvalError> {
    date_part_with_context(
        interval,
        date,
        first_day,
        first_week,
        WeekContext::default(),
    )
}

/// Apply the same Gregorian profile with caller-issued system week settings.
/// Concrete SQL options override context; omitted options keep their documented defaults.
pub fn date_part_with_context(
    interval: &Value,
    date: &Value,
    first_day: Option<&Value>,
    first_week: Option<&Value>,
    context: WeekContext,
) -> Result<Value, EvalError> {
    date_part_with_converter(
        interval,
        date,
        first_day,
        first_week,
        context,
        super::weeks::long,
    )
}

pub(crate) fn date_part_with_converter(
    interval: &Value,
    date: &Value,
    first_day: Option<&Value>,
    first_week: Option<&Value>,
    context: WeekContext,
    convert: impl FnMut(&Value) -> Result<i32, EvalError>,
) -> Result<Value, EvalError> {
    let Value::Text(interval) = interval else {
        return Err(EvalError::TypeMismatch);
    };
    let interval = Interval::resolve(interval).ok_or(EvalError::InvalidDateInterval)?;
    let settings = WeekSettings::parse_with(first_day, first_week, context, convert)?;
    if matches!(date, Value::Null) {
        return Ok(Value::Null);
    }
    let civil = conversion::calendar(date)?;
    Ok(Value::Int16(match interval {
        Interval::Year => civil.year(),
        Interval::Quarter => i16::from((civil.month() - 1) / MONTHS_PER_QUARTER + 1),
        Interval::Month => i16::from(civil.month()),
        Interval::DayOfYear => civil.date().day_of_year(),
        Interval::Day => i16::from(civil.day()),
        Interval::Weekday => settings.weekday(civil.date()),
        Interval::Week => settings.week_number(civil.date())?,
        Interval::Hour => i16::from(civil.hour()),
        Interval::Minute => i16::from(civil.minute()),
        Interval::Second => i16::from(civil.second()),
    }))
}
#[cfg(test)]
mod tests;
