//! Gregorian Format date masks use the shared input, calendar and week policies.
mod render;
mod scanner;

use super::super::{
    date_format::{self, NamedDateFormat},
    output::{NumericBuffer, TextOutput},
};
use crate::{
    EvalError,
    functions::dates::{
        WeekSettings,
        parsing::{DateConversion, DateParseContext, DateTextFormat},
        scalar_calendar,
    },
};
use jiff::civil::DateTime;
use scanner::{Field, Scanner};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
enum Style<'a> {
    Named(NamedDateFormat),
    Custom(&'a str),
    MediumDate,
    MediumTime,
}

pub(super) struct Pattern<'a> {
    style: Style<'a>,
    twelve_hour: bool,
}

pub(super) struct Display<'a> {
    pattern: Pattern<'a>,
    civil: DateTime,
    profile: Option<DateTextFormat>,
    weeks: WeekSettings,
}

impl<'a> Pattern<'a> {
    pub fn parse(
        mask: &'a str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Option<Self>, EvalError> {
        for (name, mode) in [
            ("General Date", NamedDateFormat::General),
            ("Long Date", NamedDateFormat::LongDate),
            ("Short Date", NamedDateFormat::ShortDate),
            ("Long Time", NamedDateFormat::LongTime),
            ("Short Time", NamedDateFormat::ShortTime),
        ] {
            if mask.eq_ignore_ascii_case(name) {
                return Ok(Some(Self {
                    style: Style::Named(mode),
                    twelve_hour: false,
                }));
            }
        }
        let style = if mask.eq_ignore_ascii_case("Medium Date") {
            Style::MediumDate
        } else if mask.eq_ignore_ascii_case("Medium Time") {
            Style::MediumTime
        } else if scanner::contains_date(mask, step)? {
            Style::Custom(mask)
        } else {
            return Ok(None);
        };
        let mut twelve_hour = false;
        if let Style::Custom(mask) = style {
            let mut scanner = Scanner::new(mask);
            while let Some(field) = scanner.next(step)? {
                twelve_hour |= matches!(field, Field::Meridiem(_));
            }
        }
        Ok(Some(Self { style, twelve_hour }))
    }

    pub fn resolve(
        self,
        value: &Value,
        context: DateParseContext,
        weeks: WeekSettings,
        scratch: &mut impl NumericBuffer,
    ) -> Result<Display<'a>, EvalError> {
        scratch.step()?;
        let civil = match value {
            Value::DateTime(_) | Value::DateTimeExtended(_) => scalar_calendar(value)?,
            value => scalar_calendar(&DateConversion::CDate.apply(value, context, scratch)?)?,
        };
        Ok(Display {
            pattern: self,
            civil: date_format::rounded(civil)?,
            profile: context.date_format,
            weeks,
        })
    }
}

impl Display<'_> {
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        render::write(self, output)
    }
}

#[cfg(test)]
mod tests;
