//! MS-VBAL date-token priorities; two-digit years are lexical constants.
use super::super::{ReferenceYear, date::named_month};
use crate::EvalError;
use jiff::civil::Date;

const MAX_COMPONENTS: usize = 3;
const YEAR_CENTURY: u16 = 100;
const MODERN_YEAR_BOUNDARY: u16 = 30;
const MODERN_CENTURY: u16 = 2000;
const PREVIOUS_CENTURY: u16 = 1900;
const MIN_YEAR: u16 = 100;
const MAX_YEAR: u16 = 9999;
const MAX_MONTH: u16 = 12;
const LEAP_VALIDATION_YEAR: u16 = 2000;

#[derive(Clone, Copy)]
enum Component {
    Number(u16),
    Month(u16),
}

pub(super) fn parse(value: &str, reference_year: Option<ReferenceYear>) -> Result<Date, EvalError> {
    let (parts, count) = components(value)?;

    match &parts[..count] {
        [
            Component::Number(left),
            Component::Number(middle),
            Component::Number(right),
        ] => checked(year(*right), *left, *middle)
            .or_else(|_| checked(year(*left), *middle, *right))
            .or_else(|_| checked(year(*right), *middle, *left)),
        [left, middle, right] => {
            let mut month = None;
            let mut numbers = [0; 2];
            let mut count = 0;
            for part in [left, middle, right] {
                match part {
                    Component::Month(value) if month.is_none() => month = Some(*value),
                    Component::Number(value) if count < numbers.len() => {
                        numbers[count] = *value;
                        count += 1;
                    }
                    _ => return Err(EvalError::UnsupportedLiteral),
                }
            }
            let month = month.ok_or(EvalError::UnsupportedLiteral)?;
            let [first, second] = numbers;
            checked(year(second), month, first).or_else(|_| checked(year(first), month, second))
        }
        [Component::Number(left), Component::Number(right)] => {
            if let Some(year) = reference_year {
                if let Ok(date) = checked(year.year(), *left, *right)
                    .or_else(|_| checked(year.year(), *right, *left))
                {
                    return Ok(date);
                }
            } else if checked(LEAP_VALIDATION_YEAR, *left, *right).is_ok()
                || checked(LEAP_VALIDATION_YEAR, *right, *left).is_ok()
            {
                return Err(EvalError::ReferenceYearRequired);
            }
            if (1..=MAX_MONTH).contains(left) {
                checked(*right, *left, 1)
            } else {
                checked(*left, *right, 1)
            }
        }
        [Component::Month(month), Component::Number(value)]
        | [Component::Number(value), Component::Month(month)] => {
            if let Some(year) = reference_year {
                if let Ok(date) = checked(year.year(), *month, *value) {
                    return Ok(date);
                }
            } else if checked(LEAP_VALIDATION_YEAR, *month, *value).is_ok() {
                return Err(EvalError::ReferenceYearRequired);
            }
            checked(year(*value), *month, 1)
        }
        _ => Err(EvalError::UnsupportedLiteral),
    }
}

fn year(value: u16) -> u16 {
    if value < MODERN_YEAR_BOUNDARY {
        MODERN_CENTURY + value
    } else if value < YEAR_CENTURY {
        PREVIOUS_CENTURY + value
    } else {
        value
    }
}

fn checked(year: u16, month: u16, day: u16) -> Result<Date, EvalError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
        return Err(EvalError::UnsupportedLiteral);
    }
    Date::new(
        year as i16,
        month
            .try_into()
            .map_err(|_| EvalError::UnsupportedLiteral)?,
        day.try_into().map_err(|_| EvalError::UnsupportedLiteral)?,
    )
    .map_err(|_| EvalError::UnsupportedLiteral)
}

fn components(value: &str) -> Result<([Component; MAX_COMPONENTS], usize), EvalError> {
    let mut result = [Component::Number(0); MAX_COMPONENTS];
    let mut count = 0;
    let mut remaining = value.trim();
    loop {
        let end = remaining
            .find(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '/' | '-' | ','))
            .unwrap_or(remaining.len());
        let token = &remaining[..end];
        if token.is_empty() || count == MAX_COMPONENTS {
            return Err(EvalError::UnsupportedLiteral);
        }

        result[count] = if token.bytes().all(|byte| byte.is_ascii_digit()) {
            Component::Number(token.parse().map_err(|_| EvalError::UnsupportedLiteral)?)
        } else {
            Component::Month(named_month(token).map_err(|_| EvalError::UnsupportedLiteral)?)
        };
        count += 1;

        remaining = remaining[end..].trim_start();
        if remaining.is_empty() {
            break;
        }
        if remaining.starts_with(['/', '-', ',']) {
            remaining = remaining[1..].trim_start();
            if remaining.is_empty() {
                return Err(EvalError::UnsupportedLiteral);
            }
        }
    }

    Ok((result, count))
}
