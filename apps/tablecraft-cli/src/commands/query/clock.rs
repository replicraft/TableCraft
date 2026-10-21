//! CLI-owned local clock and deterministic civil-reading override.
use jiff::{Timestamp, civil::DateTime, tz::TimeZone};
use std::{sync::OnceLock, time::SystemTime};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};

const MIN_CLOCK_TEXT_BYTES: usize = "0100-01-01T00:00:00".len();
const MAX_CLOCK_TEXT_BYTES: usize = "9999-12-31T23:59:59.123456789".len();
const DATE_BYTES: usize = "9999-12-31".len();
const YEAR_SEPARATOR: usize = "9999".len();
const MONTH_SEPARATOR: usize = "9999-12".len();
const HOUR_SEPARATOR: usize = "9999-12-31T23".len();
const MINUTE_SEPARATOR: usize = "9999-12-31T23:59".len();
const MIN_FRACTION_BYTES: usize = ".0".len();

pub(super) struct HostClock {
    fixed: Option<ClockReading>,
    zone: OnceLock<Result<TimeZone, ()>>,
}
impl HostClock {
    pub fn new(fixed: Option<ClockReading>) -> Self {
        Self {
            fixed,
            zone: OnceLock::new(),
        }
    }
}
impl Clock for HostClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if let Some(reading) = self.fixed {
            return Ok(reading);
        }
        let zone = self
            .zone
            .get_or_init(|| TimeZone::try_system().map_err(|_| ()))
            .as_ref()
            .map_err(|_| EvalError::ClockUnavailable)?;
        let instant =
            Timestamp::try_from(SystemTime::now()).map_err(|_| EvalError::ClockUnavailable)?;
        reading(instant.to_zoned(zone.clone()).datetime())
    }
}
fn reading(value: DateTime) -> Result<ClockReading, EvalError> {
    let value = ExtendedDateTime::new(
        value.year().try_into().map_err(|_| EvalError::Overflow)?,
        value.month() as u8,
        value.day() as u8,
        value.hour() as u8,
        value.minute() as u8,
        value.second() as u8,
        0,
    )
    .map_err(|_| EvalError::Overflow)?;
    ClockReading::new(value)
}
pub(super) fn parse_reading(text: &str) -> Result<ClockReading, String> {
    let parse = || {
        let length = text.len();
        if length != MIN_CLOCK_TEXT_BYTES
            && !(MIN_CLOCK_TEXT_BYTES + MIN_FRACTION_BYTES..=MAX_CLOCK_TEXT_BYTES).contains(&length)
        {
            return None;
        }
        if !text.bytes().enumerate().all(|(index, byte)| match index {
            YEAR_SEPARATOR | MONTH_SEPARATOR => byte == b'-',
            DATE_BYTES => byte == b'T',
            HOUR_SEPARATOR | MINUTE_SEPARATOR => byte == b':',
            MIN_CLOCK_TEXT_BYTES => byte == b'.',
            _ => byte.is_ascii_digit(),
        }) {
            return None;
        }
        reading(text.parse::<DateTime>().ok()?).ok()
    };
    parse().ok_or_else(|| {
        "expected local YYYY-MM-DDTHH:MM:SS[.fraction] in years 0100..9999, without an offset"
            .into()
    })
}
