//! Round the original spelling directly into the shared 96-bit Decimal envelope.
use super::scaled;
use crate::EvalError;
use tablecraft_core::values::{Decimal, Value};

pub(super) fn convert(text: &str) -> Result<Value, EvalError> {
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let exponent = scaled::bounded_exponent(exponent)?;
    let mantissa = mantissa.trim_start_matches('-');
    let whole = mantissa.find('.').unwrap_or(mantissa.len()) as i64;
    let digits = mantissa.bytes().filter(u8::is_ascii_digit);
    let leading = digits.clone().take_while(|digit| *digit == b'0').count() as i64;
    let fractional = digits.count() as i64 - whole;
    let integer_digits = whole.saturating_add(exponent).saturating_sub(leading);
    let available = i64::from(Decimal::MAX_PRECISION)
        .saturating_sub(integer_digits)
        .clamp(0, i64::from(Decimal::MAX_SCALE));
    let mut scale = fractional.saturating_sub(exponent).clamp(0, available) as u8;
    loop {
        let coefficient = scaled::rounded(text, i64::from(scale), true)?;
        if coefficient.abs() <= Decimal::MAX_COEFFICIENT {
            return Decimal::new(coefficient, scale)
                .map(Value::Decimal)
                .map_err(|_| EvalError::Overflow);
        }
        if scale == 0 {
            return Err(EvalError::Overflow);
        }
        scale -= 1;
    }
}
