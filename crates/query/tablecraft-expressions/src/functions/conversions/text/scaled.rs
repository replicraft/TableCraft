//! Shared exact lexical scale/range checks; arithmetic stays in the numeric kernel.
use crate::EvalError;
use std::num::IntErrorKind;
const DECIMAL_RADIX: u128 = 10;
const HALF_DIGIT: u8 = 5;

// Exact lexical scale/range checks read decimal digits, not an already rounded
// float. The caller charges bounded normalization/scans before this pass.
pub(crate) fn rounded(text: &str, scale: i64, round: bool) -> Result<i128, EvalError> {
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches('-');
    let digits = mantissa.bytes().filter(u8::is_ascii_digit);
    let digit_count = digits.clone().count();
    let nonzero = digits.clone().any(|digit| digit != b'0');
    if !nonzero {
        return Ok(0);
    }
    let exponent = bounded_exponent(exponent)?;
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

// Grammar validation precedes this call. Classify unrepresentably large exponents
// explicitly; magnitude saturation is enough to decide zero/overflow without
// expanding powers or hiding malformed exponent text behind a fallback.
pub(super) fn bounded_exponent(text: &str) -> Result<i64, EvalError> {
    match text.parse::<i64>() {
        Ok(value) => Ok(value),
        Err(error) => match error.kind() {
            IntErrorKind::PosOverflow => Ok(i64::MAX),
            IntErrorKind::NegOverflow => Ok(i64::MIN),
            _ => Err(EvalError::TypeMismatch),
        },
    }
}
