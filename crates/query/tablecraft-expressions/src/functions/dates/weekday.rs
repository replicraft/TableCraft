//! Standalone Gregorian Weekday with explicit date parsing and host week policy.
use super::{
    conversion,
    parsing::{DateConversion, DateParseContext},
    weeks::{WeekContext, WeekSettings},
};
use crate::{EvalError, functions::text::output::NumericBuffer};
use tablecraft_core::values::Value;

/// Return a relative Gregorian weekday as Int16, or propagate a NULL date.
/// Omitted first-day means Sunday; zero resolves the caller's host first-day.
/// Numeric/Text inputs reuse CDate's checked profile. Extended dates retain their
/// components. Hosts must evaluate both written arguments before calling.
/// The initially empty scratch buffer owns charged numeric Text normalization.
pub fn weekday(
    value: &Value,
    first_day: Option<&Value>,
    week_context: WeekContext,
    date_context: DateParseContext,
    scratch: &mut impl NumericBuffer,
) -> Result<Value, EvalError> {
    scratch.step()?;
    if !scratch.text().is_empty() {
        return Err(EvalError::Domain);
    }
    let settings = WeekSettings::parse(first_day, None, week_context)?;
    if matches!(value, Value::Null) {
        return Ok(Value::Null);
    }
    let civil = match value {
        Value::DateTime(_) | Value::DateTimeExtended(_) => conversion::calendar(value)?,
        value => {
            let converted = DateConversion::CDate.apply(value, date_context, scratch)?;
            conversion::calendar(&converted)?
        }
    };
    Ok(Value::Int16(settings.weekday(civil.date())))
}
#[cfg(test)]
mod tests;
