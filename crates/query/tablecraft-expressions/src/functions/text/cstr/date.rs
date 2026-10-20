//! Explicit Gregorian short-date/long-time spellings, without ambient locale access.
use super::super::{format_buffer::FormatBuffer, output::TextOutput};
use crate::{EvalError, functions::dates::scalar_calendar};
use jiff::{RoundMode, Unit, civil::DateTimeRound};
use std::fmt::Write;
use tablecraft_core::values::Value;

const AUTOMATION_EPOCH: (i16, i8, i8) = (1899, 12, 30);
const HALF_DAY_HOURS: i8 = 12;

/// Explicit numeric Gregorian date/time spelling for CStr, not an OS locale alias.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateTextFormat {
    /// Four-digit year, two-digit month/day, 24-hour time.
    Iso,
    /// Month/day/four-digit-year and 12-hour time with English AM/PM.
    EnUs,
    /// Two-digit day/month, four-digit year and 24-hour time.
    DeDe,
}
impl DateTextFormat {
    /// Resolve a supported ASCII-insensitive policy name without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("iso", Self::Iso),
            ("en-us", Self::EnUs),
            ("de-de", Self::DeDe),
        ]
        .into_iter()
        .find_map(|(spelling, format)| name.eq_ignore_ascii_case(spelling).then_some(format))
    }
}
/// Borrowed CStr date/time output, emitted only under a charged host sink.
#[derive(Debug, PartialEq, Eq)]
pub struct DateText<'a> {
    value: &'a Value,
    format: DateTextFormat,
}
impl<'a> DateText<'a> {
    pub(super) fn new(value: &'a Value, format: DateTextFormat) -> Self {
        Self { value, format }
    }
    /// Format to seconds using checked Gregorian nearest-second half-expand rounding.
    /// The Automation epoch date emits time only; other midnight values emit date only.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let civil = scalar_calendar(self.value)?
            .round(
                DateTimeRound::new()
                    .smallest(Unit::Second)
                    .mode(RoundMode::HalfExpand),
            )
            .map_err(|_| EvalError::Overflow)?;
        let date = (civil.year(), civil.month(), civil.day());
        let time = (civil.hour(), civil.minute(), civil.second());
        let show_date = date != AUTOMATION_EPOCH;
        let show_time = !show_date || time != (0, 0, 0);
        let mut text = FormatBuffer::new();
        if show_date {
            match self.format {
                DateTextFormat::Iso => write!(text, "{:04}-{:02}-{:02}", date.0, date.1, date.2),
                DateTextFormat::EnUs => write!(text, "{}/{}/{:04}", date.1, date.2, date.0),
                DateTextFormat::DeDe => write!(text, "{:02}.{:02}.{:04}", date.2, date.1, date.0),
            }
            .map_err(|_| EvalError::TextLimit)?;
        }
        if show_time {
            if show_date {
                text.write_char(' ').map_err(|_| EvalError::TextLimit)?;
            }
            if self.format == DateTextFormat::EnUs {
                let hour = match time.0 % HALF_DAY_HOURS {
                    0 => HALF_DAY_HOURS,
                    hour => hour,
                };
                write!(
                    text,
                    "{hour}:{:02}:{:02} {}",
                    time.1,
                    time.2,
                    if time.0 < HALF_DAY_HOURS { "AM" } else { "PM" }
                )
            } else {
                write!(text, "{:02}:{:02}:{:02}", time.0, time.1, time.2)
            }
            .map_err(|_| EvalError::TextLimit)?;
        }
        output.append(text.text()?)
    }
}
