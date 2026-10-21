//! Fixed Gregorian profile recipes; locale discovery never occurs here.
use super::{
    super::{format_buffer::FormatBuffer, output::TextOutput},
    NamedDateFormat,
};
use crate::{EvalError, functions::dates::parsing::DateTextFormat};
use jiff::civil::{DateTime, Weekday};
use std::fmt::Write;

const AUTOMATION_EPOCH: (i16, i8, i8) = (1899, 12, 30);
const HALF_DAY_HOURS: i8 = 12;
const GERMAN_MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

pub(super) fn write(
    civil: DateTime,
    format: DateTextFormat,
    mode: NamedDateFormat,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    let date = (civil.year(), civil.month(), civil.day());
    let time = (civil.hour(), civil.minute(), civil.second());
    let show_date = match mode {
        NamedDateFormat::General => date != AUTOMATION_EPOCH,
        NamedDateFormat::LongDate | NamedDateFormat::ShortDate => true,
        _ => false,
    };
    let show_time = match mode {
        NamedDateFormat::General => !show_date || time != (0, 0, 0),
        NamedDateFormat::LongTime | NamedDateFormat::ShortTime => true,
        _ => false,
    };
    let mut text = FormatBuffer::new();
    if show_date {
        if mode == NamedDateFormat::LongDate && format == DateTextFormat::EnUs {
            write!(text, "{}", civil.strftime("%A, %B %-d, %Y"))
        } else if mode == NamedDateFormat::LongDate && format == DateTextFormat::DeDe {
            let weekday = match civil.date().weekday() {
                Weekday::Monday => "Montag",
                Weekday::Tuesday => "Dienstag",
                Weekday::Wednesday => "Mittwoch",
                Weekday::Thursday => "Donnerstag",
                Weekday::Friday => "Freitag",
                Weekday::Saturday => "Samstag",
                Weekday::Sunday => "Sonntag",
            };
            write!(
                text,
                "{weekday}, {}. {} {:04}",
                date.2,
                GERMAN_MONTHS[(date.1 - 1) as usize],
                date.0
            )
        } else {
            match format {
                DateTextFormat::Iso => write!(text, "{:04}-{:02}-{:02}", date.0, date.1, date.2),
                DateTextFormat::EnUs => write!(text, "{}/{}/{:04}", date.1, date.2, date.0),
                DateTextFormat::DeDe => write!(text, "{:02}.{:02}.{:04}", date.2, date.1, date.0),
            }
        }
        .map_err(|_| EvalError::TextLimit)?;
    }
    if show_time {
        if show_date {
            text.write_char(' ').map_err(|_| EvalError::TextLimit)?;
        }
        if mode == NamedDateFormat::ShortTime {
            write!(text, "{:02}:{:02}", time.0, time.1)
        } else if format == DateTextFormat::EnUs {
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
