//! Explicit 12/24-hour numeric time grammar, validated with Jiff.
use super::date::number;
use crate::EvalError;
use jiff::civil::Time;

const COMPONENTS: usize = 3;
const MIN_TIME_COMPONENTS: usize = 2;
pub(super) const MERIDIEM_BYTES: usize = 2;
const COMPONENT_DIGITS: usize = 2;
const NANOSECOND_DIGITS: usize = 9;
const HALF_DAY_HOURS: u16 = 12;

pub(super) fn meridiem(value: &str) -> Option<bool> {
    let suffix = value.get(value.len().checked_sub(MERIDIEM_BYTES)?..)?;
    if suffix.eq_ignore_ascii_case("AM") {
        Some(false)
    } else if suffix.eq_ignore_ascii_case("PM") {
        Some(true)
    } else {
        None
    }
}
pub(super) fn parse(value: &str) -> Result<Time, EvalError> {
    let suffix = meridiem(value);
    let value = if suffix.is_some() {
        value[..value.len() - MERIDIEM_BYTES].trim_end()
    } else {
        value
    };
    let mut pieces = value.split(':');
    let mut components = [0; COMPONENTS];
    let mut count = 0;
    let mut nanos = 0;
    for piece in &mut pieces {
        if count >= COMPONENTS {
            return Err(EvalError::TypeMismatch);
        }
        let (piece, fraction) = if count == COMPONENTS - 1 {
            piece
                .split_once('.')
                .map_or((piece, None), |(a, b)| (a, Some(b)))
        } else {
            (piece, None)
        };
        components[count] = number(piece.trim(), COMPONENT_DIGITS)?;
        if let Some(fraction) = fraction {
            if fraction.is_empty()
                || fraction.len() > NANOSECOND_DIGITS
                || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(EvalError::TypeMismatch);
            }
            nanos = fraction
                .parse::<i32>()
                .map_err(|_| EvalError::TypeMismatch)?
                * 10_i32.pow((NANOSECOND_DIGITS - fraction.len()) as u32);
        }
        count += 1;
    }
    if count < MIN_TIME_COMPONENTS && suffix.is_none() {
        return Err(EvalError::TypeMismatch);
    }
    let [mut hour, minute, second] = components;
    if let Some(pm) = suffix {
        if !(1..=HALF_DAY_HOURS).contains(&hour) {
            return Err(EvalError::TypeMismatch);
        }
        hour = hour % HALF_DAY_HOURS + if pm { HALF_DAY_HOURS } else { 0 };
    }
    Time::new(
        hour.try_into().map_err(|_| EvalError::TypeMismatch)?,
        minute.try_into().map_err(|_| EvalError::TypeMismatch)?,
        second.try_into().map_err(|_| EvalError::TypeMismatch)?,
        nanos,
    )
    .map_err(|_| EvalError::TypeMismatch)
}
