//! Gregorian constructors with an explicit deterministic two-digit-year window.
use crate::{EvalError, functions::conversions::Conversion};
use jiff::{
    Span,
    civil::{Date, Time},
};
use tablecraft_core::values::{LegacyDateTime, Value};

const MONTHS_PER_YEAR: i32 = 12;
const DAYS_PER_COMMON_YEAR: i64 = 365;
const LEAP_CYCLE: i64 = 4;
const CENTURY: i64 = 100;
const GREGORIAN_CYCLE: i64 = 400;
const ANCHOR_YEAR: i16 = 2000;
const SECONDS_PER_MINUTE: i64 = 60;
const MINUTES_PER_HOUR: i64 = 60;
const HOURS_PER_DAY: i64 = 24;
const SECONDS_PER_DAY: i64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR * HOURS_PER_DAY;
const ARGUMENTS: usize = 3;

/// Inclusive 100-year window entirely within the legacy DATE calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoDigitYearWindow {
    last: u16,
}
impl TwoDigitYearWindow {
    /// Microsoft's documented default: years 00..29 -> 2000..2029, 30..99 -> 1930..1999.
    pub const ACCESS_DEFAULT: Self = Self { last: 2029 };
    /// Choose the final year of a 100-year window, from 199 through 9999.
    pub fn new(last: u16) -> Result<Self, EvalError> {
        if !(199..=9999).contains(&last) {
            return Err(EvalError::Domain);
        }
        Ok(Self { last })
    }
    pub(super) fn resolve(self, year: i16) -> i32 {
        if !(0..100).contains(&year) {
            return i32::from(year);
        }
        let last = i32::from(self.last);
        let candidate = last / 100 * 100 + i32::from(year);
        if candidate > last {
            candidate - 100
        } else {
            candidate
        }
    }
}
/// Closed, eager numeric date/time constructors; no clock, locale parser or VBA host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateConstructor {
    /// Gregorian DateSerial(year,month,day).
    DateSerial,
    /// TimeSerial(hour,minute,second), retaining signed day carry.
    TimeSerial,
}
impl DateConstructor {
    /// Maximum number of evaluated operands retained on the stack.
    pub const MAX_ARGUMENTS: usize = ARGUMENTS;
    /// Resolve an unqualified ASCII-insensitive built-in name.
    pub fn resolve(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("DateSerial") {
            Some(Self::DateSerial)
        } else if name.eq_ignore_ascii_case("TimeSerial") {
            Some(Self::TimeSerial)
        } else {
            None
        }
    }
    /// Constructors require exactly three supplied expressions.
    pub const fn accepts_arity(self, count: usize) -> bool {
        count == ARGUMENTS
    }
    /// Apply three already evaluated arguments with the documented 1930..2029 window.
    pub fn apply(self, arguments: &[Value]) -> Result<Value, EvalError> {
        self.apply_with_window(arguments, TwoDigitYearWindow::ACCESS_DEFAULT)
    }
    /// Apply with a caller-supplied Gregorian year window. All arguments undergo
    /// checked half-even Int16 conversion; NULL is an invalid conversion. Callers
    /// evaluate every supplied expression eagerly before entering this kernel.
    pub fn apply_with_window(
        self,
        arguments: &[Value],
        window: TwoDigitYearWindow,
    ) -> Result<Value, EvalError> {
        self.apply_with_components(arguments.len(), window, |index| {
            match Conversion::Integer.apply(&arguments[index])? {
                Value::Int16(value) => Ok(value),
                _ => Err(EvalError::TypeMismatch),
            }
        })
    }

    pub(crate) fn apply_with_components(
        self,
        count: usize,
        window: TwoDigitYearWindow,
        mut component: impl FnMut(usize) -> Result<i16, EvalError>,
    ) -> Result<Value, EvalError> {
        if !self.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        let mut components = [0_i16; ARGUMENTS];
        for (index, slot) in components.iter_mut().enumerate() {
            *slot = component(index)?;
        }
        let [a, b, c] = components;
        let date = match self {
            Self::DateSerial => {
                let month = i32::from(b) - 1;
                let year = window.resolve(a) + month.div_euclid(MONTHS_PER_YEAR);
                let month = (month.rem_euclid(MONTHS_PER_YEAR) + 1) as i8;
                // A cycle-equivalent year lets Jiff supply the month ordinal even
                // when relative inputs temporarily exceed its calendar envelope.
                let cycle_year = i64::from(year).rem_euclid(GREGORIAN_CYCLE) as i16 + ANCHOR_YEAR;
                let ordinal = Date::new(cycle_year, month, 1)
                    .map_err(|_| EvalError::Overflow)?
                    .day_of_year()
                    - 1;
                let days = before_year(i64::from(year)) - before_year(i64::from(ANCHOR_YEAR))
                    + i64::from(ordinal)
                    + i64::from(c)
                    - 1;
                let date = Date::new(ANCHOR_YEAR, 1, 1)
                    .map_err(|_| EvalError::Overflow)?
                    .checked_add(
                        Span::new()
                            .try_days(days)
                            .map_err(|_| EvalError::Overflow)?,
                    )
                    .map_err(|_| EvalError::Overflow)?;
                LegacyDateTime::from_calendar(date.to_datetime(Time::MIN))
                    .map_err(|_| EvalError::Overflow)?
            }
            Self::TimeSerial => {
                let seconds = (i64::from(a) * MINUTES_PER_HOUR + i64::from(b)) * SECONDS_PER_MINUTE
                    + i64::from(c);
                LegacyDateTime::from_ole_days(seconds as f64 / SECONDS_PER_DAY as f64)
                    .map_err(|_| EvalError::Overflow)?
            }
        };
        Ok(Value::DateTime(date))
    }
}
fn before_year(year: i64) -> i64 {
    let previous = year - 1;
    DAYS_PER_COMMON_YEAR * previous + previous.div_euclid(LEAP_CYCLE) - previous.div_euclid(CENTURY)
        + previous.div_euclid(GREGORIAN_CYCLE)
}

#[cfg(test)]
mod tests;
