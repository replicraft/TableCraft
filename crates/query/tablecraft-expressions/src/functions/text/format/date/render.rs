//! Stream intrinsic date fields and explicit regional fragments without heap scratch.
use super::{
    Display, Style,
    scanner::{Field, Meridiem, Scanner},
};
use crate::{
    EvalError,
    functions::{
        dates::parsing::DateTextFormat,
        text::{
            calendar_names::CalendarName,
            date_format::{self, NamedDateFormat},
            format_buffer::FormatBuffer,
            output::TextOutput,
        },
    },
};
use std::fmt::Write;

const HALF_DAY_HOURS: i8 = 12;
const MONTHS_PER_QUARTER: i8 = 3;
const TWO_DIGIT_YEAR_MODULUS: i16 = 100;
const YEAR_WIDTH: usize = 4;
const PADDED_WIDTH: usize = 2;

pub(super) fn write(display: &Display<'_>, output: &mut impl TextOutput) -> Result<(), EvalError> {
    let civil = display.civil;
    let mask = match display.pattern.style {
        Style::Named(mode) => {
            let profile = if mode == NamedDateFormat::ShortTime {
                display.profile.unwrap_or(DateTextFormat::Iso)
            } else {
                profile(display)?
            };
            return date_format::write_civil(civil, profile, mode, output);
        }
        Style::MediumDate => "dd-mmm-yy",
        Style::MediumTime => {
            if profile(display)? != DateTextFormat::EnUs {
                return Err(EvalError::UnsupportedExpression);
            }
            "hh:nn AM/PM"
        }
        Style::Custom(mask) => mask,
    };
    let twelve_hour =
        display.pattern.twelve_hour || matches!(display.pattern.style, Style::MediumTime);
    let mut scanner = Scanner::new(mask);
    while let Some(field) = scanner.next(&mut || output.step())? {
        match field {
            Field::Literal(text) => output.append(text)?,
            Field::Punctuation(character) => {
                let mut buffer = [0; char::MAX.len_utf8()];
                output.append(character.encode_utf8(&mut buffer))?;
            }
            Field::DateSeparator => output.append(match profile(display)? {
                DateTextFormat::EnUs => "/",
                DateTextFormat::DeDe => ".",
                DateTextFormat::Iso => "-",
            })?,
            Field::TimeSeparator => {
                profile(display)?;
                output.append(":")?;
            }
            Field::Day(count @ (1 | 2)) => number(i32::from(civil.day()), count, output)?,
            Field::Day(count @ (3 | 4)) => {
                CalendarName::for_date(civil.date(), false, count == 3, profile(display)?)
                    .write(output)?
            }
            Field::Day(count @ (5 | 6)) => date_format::write_civil(
                civil,
                profile(display)?,
                if count == 5 {
                    NamedDateFormat::ShortDate
                } else {
                    NamedDateFormat::LongDate
                },
                output,
            )?,
            Field::Day(_) => return Err(EvalError::UnsupportedExpression),
            Field::Month(count @ (1 | 2)) => number(i32::from(civil.month()), count, output)?,
            Field::Month(count @ (3 | 4)) => {
                CalendarName::for_date(civil.date(), true, count == 3, profile(display)?)
                    .write(output)?
            }
            Field::Month(_) => return Err(EvalError::UnsupportedExpression),
            Field::Year(1) => number(i32::from(civil.date().day_of_year()), 1, output)?,
            Field::Year(2) => number(
                i32::from(civil.year() % TWO_DIGIT_YEAR_MODULUS),
                PADDED_WIDTH,
                output,
            )?,
            Field::Year(4) => number(i32::from(civil.year()), YEAR_WIDTH, output)?,
            Field::Year(_) => return Err(EvalError::UnsupportedExpression),
            Field::Hour(width) => {
                let hour = if twelve_hour {
                    match civil.hour() % HALF_DAY_HOURS {
                        0 => HALF_DAY_HOURS,
                        hour => hour,
                    }
                } else {
                    civil.hour()
                };
                number(i32::from(hour), width, output)?;
            }
            Field::Minute(width) => number(i32::from(civil.minute()), width, output)?,
            Field::Second(width) => number(i32::from(civil.second()), width, output)?,
            Field::Weekday => number(i32::from(display.weeks.weekday(civil.date())), 1, output)?,
            Field::Week => number(
                i32::from(display.weeks.week_number(civil.date())?),
                1,
                output,
            )?,
            Field::Quarter => number(
                i32::from((civil.month() - 1) / MONTHS_PER_QUARTER + 1),
                1,
                output,
            )?,
            Field::General => date_format::write_civil(
                civil,
                profile(display)?,
                NamedDateFormat::General,
                output,
            )?,
            Field::LongTime => date_format::write_civil(
                civil,
                profile(display)?,
                NamedDateFormat::LongTime,
                output,
            )?,
            Field::Meridiem(style) => {
                let afternoon = civil.hour() >= HALF_DAY_HOURS;
                let text = match (style, afternoon) {
                    (Meridiem::Upper, false) => "AM",
                    (Meridiem::Upper, true) => "PM",
                    (Meridiem::Lower, false) => "am",
                    (Meridiem::Lower, true) => "pm",
                    (Meridiem::UpperInitial, false) => "A",
                    (Meridiem::UpperInitial, true) => "P",
                    (Meridiem::LowerInitial, false) => "a",
                    (Meridiem::LowerInitial, true) => "p",
                    (Meridiem::Regional, afternoon) => {
                        if profile(display)? != DateTextFormat::EnUs {
                            return Err(EvalError::UnsupportedExpression);
                        }
                        if afternoon { "PM" } else { "AM" }
                    }
                };
                output.append(text)?;
            }
        }
    }
    Ok(())
}

fn number(value: i32, width: usize, output: &mut impl TextOutput) -> Result<(), EvalError> {
    output.step()?;
    let mut text = FormatBuffer::new();
    write!(text, "{value:0width$}").map_err(|_| EvalError::TextLimit)?;
    output.append(text.text()?)
}

fn profile(display: &Display<'_>) -> Result<DateTextFormat, EvalError> {
    display.profile.ok_or(EvalError::DateFormatRequired)
}
