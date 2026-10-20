//! Checked radix accumulation and explicit signed-width interpretation.
use super::{NumericBuffer, scan::Cursor, suffix};
use crate::EvalError;
use tablecraft_core::values::Double;
const HEX_RADIX: u32 = 16;
const OCTAL_RADIX: u32 = 8;

pub(super) fn parse(
    cursor: &mut Cursor<'_>,
    negative: bool,
    buffer: &mut impl NumericBuffer,
) -> Result<Double, EvalError> {
    let radix = match cursor.next(buffer)? {
        Some('h' | 'H') => HEX_RADIX,
        Some('o' | 'O') => OCTAL_RADIX,
        _ => return Ok(Double::from_float(0.0)),
    };
    let mut value = 0u64;
    let mut count = 0;
    let mut next = cursor.next(buffer)?;
    while let Some(digit) = next
        .and_then(|ch| ch.to_digit(radix))
        .filter(|_| next.is_some_and(|ch| ch.is_ascii()))
    {
        value = value
            .checked_mul(u64::from(radix))
            .and_then(|v| v.checked_add(u64::from(digit)))
            .ok_or(EvalError::Overflow)?;
        count += 1;
        next = cursor.next(buffer)?;
    }
    if count == 0 {
        return Ok(Double::from_float(0.0));
    }
    let signed = match next {
        Some('%') => i128::from(u16::try_from(value).map_err(|_| EvalError::Overflow)? as i16),
        Some('&') => i128::from(u32::try_from(value).map_err(|_| EvalError::Overflow)? as i32),
        Some('^') => i128::from(value as i64),
        _ if value <= u64::from(u16::MAX) => i128::from(value as u16 as i16),
        _ => i128::from(u32::try_from(value).map_err(|_| EvalError::Overflow)? as i32),
    };
    let signed = if negative { -signed } else { signed };
    suffix::radix(signed, next)
}
