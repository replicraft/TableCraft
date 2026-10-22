//! Explicit Access week settings and documented legacy week-number behavior.
use super::{FirstDayOfWeek, FirstWeekOfYear};
use crate::{
    EvalError,
    numeric::{self, IntegerType},
};
use jiff::civil::Date;
use tablecraft_core::values::Value;

const DAYS_PER_WEEK: i32 = 7;
const FIRST_FOUR_DAYS: i32 = 4;
const DEFAULT_OPTION: i32 = 1;
const SYSTEM_OPTION: i32 = 0;
const MONDAY_OPTION: i32 = 2;
const JANUARY_FIRST_RULE: i32 = 1;
const FOUR_DAY_RULE: i32 = 2;
const FULL_WEEK_RULE: i32 = 3;

/// Host-owned settings used only for explicit system (zero) week options.
/// Omitted DatePart/DateDiff options retain Sunday and Jan-1 defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeekContext {
    /// Concrete first day when the caller requests the system setting.
    pub first_day: Option<FirstDayOfWeek>,
    /// Concrete first-week rule when the caller requests the system setting.
    pub first_week: Option<FirstWeekOfYear>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::functions) struct WeekSettings {
    pub(super) first_day: i32,
    first_week: i32,
}
impl WeekSettings {
    pub(in crate::functions) fn parse(
        first_day: Option<&Value>,
        first_week: Option<&Value>,
        context: WeekContext,
    ) -> Result<Self, EvalError> {
        Ok(Self {
            first_day: option(
                first_day,
                DAYS_PER_WEEK,
                context.first_day.map(|day| i32::from(day.number())),
            )?,
            first_week: option(
                first_week,
                FULL_WEEK_RULE,
                context.first_week.map(|rule| i32::from(rule.number())),
            )?,
        })
    }
    pub(in crate::functions) fn weekday(&self, date: Date) -> i16 {
        ((i32::from(date.weekday().to_sunday_zero_offset()) - (self.first_day - DEFAULT_OPTION))
            .rem_euclid(DAYS_PER_WEEK)
            + 1) as i16
    }
    pub(in crate::functions) fn week_number(&self, date: Date) -> Result<i16, EvalError> {
        let year = date.year();
        let first = Date::new(year, 1, 1).map_err(|_| EvalError::Overflow)?;
        let offset = i32::from(self.weekday(first) - 1);
        let ordinal = i32::from(date.day_of_year() - 1);
        let start = self.start(offset);
        if ordinal < start {
            // Jiff's proleptic calendar admits year zero for the prior-year
            // calculation at the Extended carrier's year-one boundary.
            let previous = Date::new(year - 1, 1, 1).map_err(|_| EvalError::Overflow)?;
            let previous_days = days_in_year(year - 1)?;
            let previous_start = self.start(i32::from(self.weekday(previous) - 1));
            return Ok(((ordinal + previous_days - previous_start) / DAYS_PER_WEEK + 1) as i16);
        }
        if self.first_week == FOUR_DAY_RULE {
            let days = days_in_year(year)?;
            let next_offset = (offset + days).rem_euclid(DAYS_PER_WEEK);
            let next_start = days + self.start(next_offset);
            if ordinal >= next_start {
                // Microsoft documents the final Monday returning week 53 while
                // the following Tuesday/Wednesday return week 1 in these years.
                let documented_monday = self.first_day == MONDAY_OPTION && self.weekday(date) == 1;
                if !documented_monday {
                    return Ok(1);
                }
            }
        }
        Ok(((ordinal - start) / DAYS_PER_WEEK + 1) as i16)
    }
    fn start(&self, offset: i32) -> i32 {
        match self.first_week {
            JANUARY_FIRST_RULE => -offset,
            FOUR_DAY_RULE if DAYS_PER_WEEK - offset >= FIRST_FOUR_DAYS => -offset,
            FULL_WEEK_RULE if offset == 0 => 0,
            _ => DAYS_PER_WEEK - offset,
        }
    }
}
fn days_in_year(year: i16) -> Result<i32, EvalError> {
    Date::new(year, 12, 31)
        .map(|date| i32::from(date.day_of_year()))
        .map_err(|_| EvalError::Overflow)
}
fn option(value: Option<&Value>, maximum: i32, system: Option<i32>) -> Result<i32, EvalError> {
    let Some(value) = value else {
        return Ok(DEFAULT_OPTION);
    };
    let Value::Int32(value) = numeric::coerce_integer(value, IntegerType::Int32)? else {
        return Err(EvalError::TypeMismatch);
    };
    if value == SYSTEM_OPTION {
        return system.ok_or(EvalError::CalendarPolicyRequired);
    }
    if !(DEFAULT_OPTION..=maximum).contains(&value) {
        return Err(EvalError::Domain);
    }
    Ok(value)
}
