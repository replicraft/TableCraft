//! Bounded numeric date order and Jiff's English full/abbreviated month recognition.
use super::{DateParseContext, DateTextFormat, ReferenceYear};
use crate::EvalError;
use jiff::{civil::Date, fmt::strtime};

const COMPONENTS: usize = 3;
const LEAP_VALIDATION_YEAR: i16 = 2000;
const MAX_MONTH_BYTES: usize = "September".len();
const YEAR_DIGITS: usize = 4;
const MONTH_DAY_DIGITS: usize = 2;

pub(super) fn parse(value: &str, context: DateParseContext) -> Result<Date, EvalError> {
    let format = context.date_format.ok_or(EvalError::DateFormatRequired)?;
    let named = value.bytes().any(|byte| byte.is_ascii_alphabetic());
    if named {
        let tokens = tokens(
            value
                .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '-')
                .filter(|token| !token.is_empty()),
        )?;
        let (year, month, day) = match tokens.as_slice() {
            [a, b, c] => {
                let (month, day) = month_day(a, b)?;
                (year(c, context)?, month, day)
            }
            [a, b] => {
                let (month, day) = month_day(a, b)?;
                (omitted_year(month, day, context)?, month, day)
            }
            _ => return Err(EvalError::TypeMismatch),
        };
        return checked_date(year, month, day);
    }
    let iso = value.split('-').next().is_some_and(|first| {
        first.len() == YEAR_DIGITS && first.bytes().all(|byte| byte.is_ascii_digit())
    });
    let separator = if iso || format == DateTextFormat::Iso {
        '-'
    } else if format == DateTextFormat::EnUs {
        '/'
    } else {
        '.'
    };
    let parts = tokens(value.split(separator).map(str::trim))?;
    let (year, month, day) = match parts.as_slice() {
        [a, b, c] if iso || format == DateTextFormat::Iso => (
            year(a, context)?,
            number(b, MONTH_DAY_DIGITS)?,
            number(c, MONTH_DAY_DIGITS)?,
        ),
        [a, b, c] if format == DateTextFormat::EnUs => (
            year(c, context)?,
            number(a, MONTH_DAY_DIGITS)?,
            number(b, MONTH_DAY_DIGITS)?,
        ),
        [a, b, c] => (
            year(c, context)?,
            number(b, MONTH_DAY_DIGITS)?,
            number(a, MONTH_DAY_DIGITS)?,
        ),
        [a, b] => {
            let first = number(a, MONTH_DAY_DIGITS)?;
            let second = number(b, MONTH_DAY_DIGITS)?;
            let (month, day) = if format == DateTextFormat::DeDe {
                (second, first)
            } else {
                (first, second)
            };
            (omitted_year(month, day, context)?, month, day)
        }
        _ => return Err(EvalError::TypeMismatch),
    };
    checked_date(year, month, day)
}
// A tiny borrowed token array avoids allocation even for long malformed inputs.
struct Tokens<'a> {
    values: [&'a str; COMPONENTS],
    length: usize,
}
impl Tokens<'_> {
    fn as_slice(&self) -> &[&str] {
        &self.values[..self.length]
    }
}
fn tokens<'a>(parts: impl Iterator<Item = &'a str>) -> Result<Tokens<'a>, EvalError> {
    let mut tokens = Tokens {
        values: [""; COMPONENTS],
        length: 0,
    };
    for part in parts {
        *tokens
            .values
            .get_mut(tokens.length)
            .ok_or(EvalError::TypeMismatch)? = part;
        tokens.length += 1;
    }
    Ok(tokens)
}
fn month_day(a: &str, b: &str) -> Result<(u16, u16), EvalError> {
    let (month, day) = if a.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        (a, b)
    } else {
        (b, a)
    };
    if month.len() > MAX_MONTH_BYTES {
        return Err(EvalError::TypeMismatch);
    }
    let parsed = strtime::parse("%B", month)
        .or_else(|_| strtime::parse("%b", month))
        .map_err(|_| EvalError::TypeMismatch)?;
    Ok((
        parsed.month().ok_or(EvalError::TypeMismatch)? as u16,
        number(day, MONTH_DAY_DIGITS)?,
    ))
}
fn year(value: &str, context: DateParseContext) -> Result<i16, EvalError> {
    let year = number(value, YEAR_DIGITS)?;
    let year = if value.len() <= MONTH_DAY_DIGITS {
        context.year_window.resolve(year as i16)
    } else {
        i32::from(year)
    };
    year.try_into().map_err(|_| EvalError::TypeMismatch)
}
fn omitted_year(month: u16, day: u16, context: DateParseContext) -> Result<i16, EvalError> {
    // Validate the possible Gregorian month/day before requiring a host year.
    checked_date(LEAP_VALIDATION_YEAR, month, day)?;
    reference(context)
}
fn reference(context: DateParseContext) -> Result<i16, EvalError> {
    context
        .reference_year
        .map(|year| year.year() as i16)
        .ok_or(EvalError::ReferenceYearRequired)
}
fn checked_date(year: i16, month: u16, day: u16) -> Result<Date, EvalError> {
    if !(ReferenceYear::MIN as i16..=ReferenceYear::MAX as i16).contains(&year) {
        return Err(EvalError::TypeMismatch);
    }
    Date::new(
        year,
        month.try_into().map_err(|_| EvalError::TypeMismatch)?,
        day.try_into().map_err(|_| EvalError::TypeMismatch)?,
    )
    .map_err(|_| EvalError::TypeMismatch)
}
pub(super) fn number(value: &str, width: usize) -> Result<u16, EvalError> {
    if value.is_empty() || value.len() > width || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EvalError::TypeMismatch);
    }
    value.parse().map_err(|_| EvalError::TypeMismatch)
}
