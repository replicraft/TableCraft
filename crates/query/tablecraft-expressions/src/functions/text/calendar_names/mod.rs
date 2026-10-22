//! Explicit-language month and weekday names with caller-issued week numbering.
mod labels;
use super::{TextContext, TextFunction, format_buffer::FormatBuffer, output::TextOutput};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::{FirstDayOfWeek, parsing::DateTextFormat},
    },
};
use jiff::civil::{Date, Weekday};
use std::fmt::Write;
use tablecraft_core::values::Value;

const MONTHS_PER_YEAR: i32 = 12;
const DAYS_PER_WEEK: i32 = 7;
const FIRST_ORDINAL: i32 = 1;
const SYSTEM_FIRST_DAY: i32 = 0;
const NAME_ANCHOR_YEAR: i16 = 2000;
const SUNDAY_ANCHOR: Date = Date::constant(2000, 1, 2);

/// Validated borrowed-free name recipe, written through a bounded host output.
#[derive(Debug, PartialEq, Eq)]
pub struct CalendarName {
    date: Date,
    month: bool,
    abbreviated: bool,
    profile: DateTextFormat,
}
pub(super) fn prepare(
    function: TextFunction,
    arguments: &[Value],
    context: TextContext<'_>,
) -> Result<CalendarName, EvalError> {
    let ordinal = long(&arguments[0])?;
    let abbreviated = match arguments.get(1) {
        None => false,
        Some(value) => {
            let Value::Boolean(value) = Conversion::Boolean.apply(value)? else {
                return Err(EvalError::TypeMismatch);
            };
            value
        }
    };
    let month = function == TextFunction::MonthName;
    let date = if month {
        if !(FIRST_ORDINAL..=MONTHS_PER_YEAR).contains(&ordinal) {
            return Err(EvalError::Domain);
        }
        Date::new(NAME_ANCHOR_YEAR, ordinal as i8, FIRST_ORDINAL as i8)
            .map_err(|_| EvalError::Domain)?
    } else {
        if !(FIRST_ORDINAL..=DAYS_PER_WEEK).contains(&ordinal) {
            return Err(EvalError::Domain);
        }
        let first = arguments
            .get(2)
            .map(long)
            .transpose()?
            .unwrap_or(SYSTEM_FIRST_DAY);
        let first = if first == SYSTEM_FIRST_DAY {
            context
                .first_day_of_week
                .ok_or(EvalError::CalendarPolicyRequired)?
        } else {
            FirstDayOfWeek::new(first.try_into().map_err(|_| EvalError::Domain)?)?
        };
        let offset = ((ordinal - FIRST_ORDINAL) + (i32::from(first.number()) - FIRST_ORDINAL))
            % DAYS_PER_WEEK;
        // The fixed Sunday anchor plus at most six days stays in January.
        Date::new(
            SUNDAY_ANCHOR.year(),
            SUNDAY_ANCHOR.month(),
            SUNDAY_ANCHOR.day() + offset as i8,
        )
        .map_err(|_| EvalError::Domain)?
    };
    Ok(CalendarName {
        date,
        month,
        abbreviated,
        profile: context.date_format.ok_or(EvalError::DateFormatRequired)?,
    })
}
fn long(value: &Value) -> Result<i32, EvalError> {
    let Value::Int32(value) = Conversion::Long.apply(value)? else {
        return Err(EvalError::TypeMismatch);
    };
    Ok(value)
}
impl CalendarName {
    pub(super) fn for_date(
        date: Date,
        month: bool,
        abbreviated: bool,
        profile: DateTextFormat,
    ) -> Self {
        Self {
            date,
            month,
            abbreviated,
            profile,
        }
    }

    /// Emit an English (en-us/iso) or German (de-de) standalone name.
    /// Every byte is appended through the host; formatting has no ambient locale.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        if self.profile == DateTextFormat::DeDe {
            return output.append(if self.month {
                german_month(self.date.month() as u8, self.abbreviated)
            } else {
                german_weekday(self.date.weekday(), self.abbreviated)
            });
        }
        let pattern = match (self.month, self.abbreviated) {
            (true, false) => "%B",
            (true, true) => "%b",
            (false, false) => "%A",
            (false, true) => "%a",
        };
        let mut text = FormatBuffer::new();
        write!(text, "{}", self.date.strftime(pattern)).map_err(|_| EvalError::TextLimit)?;
        output.append(text.text()?)
    }
}
pub(super) fn german_month(month: u8, abbreviated: bool) -> &'static str {
    labels::month(month, abbreviated)
}
pub(super) fn german_weekday(day: Weekday, abbreviated: bool) -> &'static str {
    labels::weekday(day, abbreviated)
}
#[cfg(test)]
mod tests;
