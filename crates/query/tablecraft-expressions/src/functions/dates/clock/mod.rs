//! Host-issued local wall-clock readings; no system time or timezone access.
use super::conversion;
use crate::EvalError;
use jiff::civil::{Date, Time};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};

const MIN_LEGACY_YEAR: u16 = 100;
const AUTOMATION_EPOCH: Date = Date::constant(1899, 12, 30);

/// Validated local Gregorian reading in the legacy DATE envelope, at whole seconds.
/// Subsecond input is truncated before projection; no timezone or instant is inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockReading(ExtendedDateTime);
impl ClockReading {
    /// Accept a validated civil reading; reject years before 100 and discard fractions.
    pub fn new(value: ExtendedDateTime) -> Result<Self, EvalError> {
        let (year, month, day) = value.date();
        let (hour, minute, second, _) = value.time();
        if year < MIN_LEGACY_YEAR {
            return Err(EvalError::Overflow);
        }
        Ok(Self(
            ExtendedDateTime::new(year, month, day, hour, minute, second, 0)
                .map_err(|_| EvalError::Overflow)?,
        ))
    }
    /// The validated local Gregorian components, with a zero fraction.
    pub const fn date_time(self) -> ExtendedDateTime {
        self.0
    }
}
/// Host clock callback, invoked only when a supported clock expression executes.
/// Implementations must enforce their own I/O/work bounds and return payload-free
/// errors. They may return fresh readings; the query layer does not cache time.
pub trait Clock: Send + Sync {
    /// Read the host-selected local date/time; never interpret database content.
    fn read(&self) -> Result<ClockReading, EvalError>;
}
impl Clock for ClockReading {
    fn read(&self) -> Result<ClockReading, EvalError> {
        Ok(*self)
    }
}
/// Closed parenthesized, zero-argument clock functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockFunction {
    /// Local date at midnight.
    Date,
    /// Local time on the Automation epoch date.
    Time,
    /// Local date and time together.
    Now,
}
impl ClockFunction {
    /// Resolve unqualified ASCII-insensitive clock names, without `$` aliases.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Date", Self::Date),
            ("Time", Self::Time),
            ("Now", Self::Now),
        ]
        .into_iter()
        .find_map(|(spelling, function)| name.eq_ignore_ascii_case(spelling).then_some(function))
    }
    /// Read once and project exact components before legacy encoding.
    /// Callers charge work/check cancellation before invoking the provider.
    pub fn apply(self, clock: Option<&dyn Clock>) -> Result<Value, EvalError> {
        let reading = clock.ok_or(EvalError::ClockRequired)?.read()?;
        let civil = conversion::extended_calendar(reading.0)?;
        let value = match self {
            Self::Date => civil.date().to_datetime(Time::MIN),
            Self::Time => AUTOMATION_EPOCH.to_datetime(civil.time()),
            Self::Now => civil,
        };
        Ok(Value::DateTime(
            LegacyDateTime::from_calendar(value).map_err(|_| EvalError::Overflow)?,
        ))
    }
}
#[cfg(test)]
mod tests;
