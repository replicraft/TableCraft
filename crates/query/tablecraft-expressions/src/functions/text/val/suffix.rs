//! Type-suffix validation, kept separate from ordinary implicit coercion.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{Currency, Double, Value};
const DECIMAL_RADIX: u128 = 10;
const HALF_DIGIT: u8 = 5;

pub(super) fn decimal(text: &str, suffix: Option<char>) -> Result<Double, EvalError> {
    match suffix {
        Some('%' | '&' | '^') => {
            let value = scaled(text, 0, false)?;
            let valid = match suffix {
                Some('%') => i16::try_from(value).is_ok(),
                Some('&') => i32::try_from(value).is_ok(),
                _ => i64::try_from(value).is_ok(),
            };
            if !valid {
                return Err(EvalError::Overflow);
            }
            Ok(Double::from_float(value as f64))
        }
        Some('@') => {
            let coefficient = i64::try_from(scaled(text, i64::from(Currency::SCALE), true)?)
                .map_err(|_| EvalError::Overflow)?;
            let Value::Double(value) =
                Conversion::Double.apply(&Value::Currency(Currency::from_scaled(coefficient)))?
            else {
                return Err(EvalError::TypeMismatch);
            };
            Ok(value)
        }
        Some('$') => Err(EvalError::TypeMismatch),
        _ => finish(
            text.parse::<f64>().map_err(|_| EvalError::TypeMismatch)?,
            suffix,
        ),
    }
}
pub(super) fn radix(value: i128, suffix: Option<char>) -> Result<Double, EvalError> {
    if suffix == Some('$') {
        return Err(EvalError::TypeMismatch);
    }
    if suffix == Some('@') {
        // The signed radix value is integral; checked Currency conversion supplies
        // the existing four-place/range policy, without a second arithmetic engine.
        let value = i64::try_from(value).map_err(|_| EvalError::Overflow)?;
        let currency = Conversion::Currency.apply(&Value::Int64(value))?;
        let Value::Double(value) = Conversion::Double.apply(&currency)? else {
            return Err(EvalError::TypeMismatch);
        };
        return Ok(value);
    }
    finish(value as f64, suffix)
}
fn finish(value: f64, suffix: Option<char>) -> Result<Double, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::Overflow);
    }
    if suffix == Some('!') {
        let single = value as f32;
        if !single.is_finite() {
            return Err(EvalError::Overflow);
        }
        return Ok(Double::from_float(f64::from(single)));
    }
    Ok(Double::from_float(value))
}
// Exact bounded integral/Currency suffix validation reads decimal digits, not an
// already rounded float. Scratch storage was charged before this second scan.
fn scaled(text: &str, scale: i64, round: bool) -> Result<i128, EvalError> {
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches('-');
    let digits = mantissa.bytes().filter(u8::is_ascii_digit);
    let digit_count = digits.clone().count();
    let nonzero = digits.clone().any(|digit| digit != b'0');
    if !nonzero {
        return Ok(0);
    }
    let exponent = match exponent.parse::<i64>() {
        Ok(value) => value,
        Err(_) if exponent.starts_with('-') => {
            return if round {
                Ok(0)
            } else {
                Err(EvalError::TypeMismatch)
            };
        }
        Err(_) => return Err(EvalError::Overflow),
    };
    let whole = mantissa.find('.').unwrap_or(mantissa.len()) as i64;
    let cut = whole.saturating_add(exponent).saturating_add(scale);
    let mut coefficient = 0u128;
    let mut first_dropped = None;
    let mut later_nonzero = false;
    for (index, digit) in digits.enumerate() {
        let digit = digit - b'0';
        if (index as i64) < cut {
            coefficient = coefficient
                .checked_mul(DECIMAL_RADIX)
                .and_then(|v| v.checked_add(u128::from(digit)))
                .ok_or(EvalError::Overflow)?;
        } else if first_dropped.is_none() && cut >= 0 {
            first_dropped = Some(digit);
        } else {
            later_nonzero |= digit != 0;
        }
    }
    if !round && (first_dropped.is_some_and(|d| d != 0) || later_nonzero) {
        return Err(EvalError::TypeMismatch);
    }
    if round
        && first_dropped.is_some_and(|d| {
            d > HALF_DIGIT || (d == HALF_DIGIT && (later_nonzero || !coefficient.is_multiple_of(2)))
        })
    {
        coefficient = coefficient.checked_add(1).ok_or(EvalError::Overflow)?;
    }
    if coefficient != 0 && cut > digit_count as i64 {
        let zeros = u32::try_from(cut - digit_count as i64).map_err(|_| EvalError::Overflow)?;
        let factor = DECIMAL_RADIX
            .checked_pow(zeros)
            .ok_or(EvalError::Overflow)?;
        coefficient = coefficient.checked_mul(factor).ok_or(EvalError::Overflow)?;
    }
    let coefficient = i128::try_from(coefficient).map_err(|_| EvalError::Overflow)?;
    Ok(if negative { -coefficient } else { coefficient })
}
