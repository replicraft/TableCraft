//! Host-issued local wall-clock readings; no system time or timezone access.
use super::conversion;
use crate::EvalError;
use jiff::civil::{Date, Time};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Single, Value};

const MIN_LEGACY_YEAR: u16 = 100;
const SECONDS_PER_MINUTE: u32 = 60;
const MINUTES_PER_HOUR: u32 = 60;
const TICKS_PER_SECOND: u32 = 10_000_000;
const AUTOMATION_EPOCH: Date = Date::constant(1899, 12, 30);

/// Validated local Gregorian reading with separate DATE and Timer projections.
/// DATE projections truncate fractions; Timer retains Single seconds since midnight.
/// No timezone or instant is inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockReading {
    date_time: ExtendedDateTime,
    timer_seconds: Single,
}

impl ClockReading {
    /// Accept a civil reading in years 100..9999, with 100-nanosecond fractions.
    pub fn new(value: ExtendedDateTime) -> Result<Self, EvalError> {
        let (year, month, day) = value.date();
        let (hour, minute, second, fraction) = value.time();
        if year < MIN_LEGACY_YEAR {
            return Err(EvalError::Overflow);
        }

        let seconds = (u32::from(hour) * MINUTES_PER_HOUR + u32::from(minute)) * SECONDS_PER_MINUTE
            + u32::from(second);
        let elapsed = f64::from(seconds) + f64::from(fraction) / f64::from(TICKS_PER_SECOND);
        let date_time = ExtendedDateTime::new(year, month, day, hour, minute, second, 0)
            .map_err(|_| EvalError::Overflow)?;

        Ok(Self {
            date_time,
            timer_seconds: Single::from_float(elapsed as f32),
        })
    }

    /// The validated local Gregorian components, with a zero fraction.
    pub const fn date_time(self) -> ExtendedDateTime {
        self.date_time
    }

    /// Single seconds since midnight, rounded to nearest with ties to even.
    /// Late-day fractions can round to 86400; an actual midnight reading is zero.
    pub const fn timer_seconds(self) -> Single {
        self.timer_seconds
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
    /// Fractional Single seconds since local midnight.
    Timer,
}

impl ClockFunction {
    /// Resolve unqualified ASCII-insensitive clock names, without `$` aliases.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Date", Self::Date),
            ("Time", Self::Time),
            ("Now", Self::Now),
            ("Timer", Self::Timer),
        ]
        .into_iter()
        .find_map(|(spelling, function)| name.eq_ignore_ascii_case(spelling).then_some(function))
    }

    /// Read once and project exact components before legacy encoding.
    /// Callers charge work/check cancellation before invoking the provider.
    pub fn apply(self, clock: Option<&dyn Clock>) -> Result<Value, EvalError> {
        let reading = clock.ok_or(EvalError::ClockRequired)?.read()?;
        let civil = conversion::extended_calendar(reading.date_time)?;
        let value = match self {
            Self::Date => civil.date().to_datetime(Time::MIN),
            Self::Time => AUTOMATION_EPOCH.to_datetime(civil.time()),
            Self::Now => civil,
            Self::Timer => return Ok(Value::Single(reading.timer_seconds)),
        };
        Ok(Value::DateTime(
            LegacyDateTime::from_calendar(value).map_err(|_| EvalError::Overflow)?,
        ))
    }
}

#[cfg(test)]
mod tests;
