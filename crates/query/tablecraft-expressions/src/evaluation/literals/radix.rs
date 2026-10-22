//! Unsuffixed hexadecimal/octal magnitudes interpreted as signed 16/32-bit values.
use crate::EvalError;
use std::num::IntErrorKind;
use tablecraft_core::values::Value;

const HEX_RADIX: u32 = 16;
const OCTAL_RADIX: u32 = 8;

pub(super) fn parse(text: &str) -> Result<Value, EvalError> {
    let (radix, digits) =
        if let Some(digits) = text.strip_prefix("&H").or_else(|| text.strip_prefix("&h")) {
            (HEX_RADIX, digits)
        } else if let Some(digits) = text.strip_prefix("&O").or_else(|| text.strip_prefix("&o")) {
            (OCTAL_RADIX, digits)
        } else {
            return Err(EvalError::UnsupportedLiteral);
        };
    if digits.is_empty()
        || !digits
            .bytes()
            .all(|digit| char::from(digit).is_digit(radix))
    {
        return Err(EvalError::UnsupportedLiteral);
    }
    let bits = u32::from_str_radix(digits, radix).map_err(|error| {
        if error.kind() == &IntErrorKind::PosOverflow {
            EvalError::Overflow
        } else {
            EvalError::UnsupportedLiteral
        }
    })?;
    Ok(if let Ok(short) = u16::try_from(bits) {
        Value::Int16(i16::from_le_bytes(short.to_le_bytes()))
    } else {
        Value::Int32(i32::from_le_bytes(bits.to_le_bytes()))
    })
}
