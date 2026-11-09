//! Explicit-language month and weekday names with caller-issued week numbering.
mod arguments;
mod labels;

use super::{format_buffer::FormatBuffer, output::TextOutput};
use crate::{EvalError, functions::dates::parsing::DateTextFormat};
pub(super) use arguments::prepare;
#[cfg(feature = "sql")]
pub(crate) use arguments::prepare_with_inputs;
use jiff::civil::{Date, Weekday};
use std::fmt::Write;

/// Validated borrowed-free name recipe, written through a bounded host output.
#[derive(Debug, PartialEq, Eq)]
pub struct CalendarName {
    date: Date,
    month: bool,
    abbreviated: bool,
    profile: DateTextFormat,
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
