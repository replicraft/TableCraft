//! Exact fixed rounding and normalized finite IEEE output, using bounded Decimal work.
use super::super::{format_buffer::FormatBuffer, numeric_format::floating, output::TextOutput};
use crate::{
    EvalError,
    numeric::decimal::{FloatConversion, from_value},
};
use rust_decimal::{Decimal, RoundingStrategy};
use std::fmt::Write;
use tablecraft_core::values::Value;

const MAX_STORED_SCALE: u32 = tablecraft_core::values::Decimal::MAX_SCALE as u32;
const DECIMAL_RADIX: u64 = 10;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::functions::text) struct Magnitude {
    pub coefficient: u128,
    /// Decimal point position relative to the coefficient's first digit.
    pub point: i32,
    pub negative: bool,
}
pub(in crate::functions::text) fn prepare(
    value: &Value,
    places: u8,
    shift: u32,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    let (coefficient, scale, negative) = match value {
        Value::Single(value) => {
            return float(
                f64::from(value.to_float()),
                floating::SINGLE_DIGITS,
                places,
                shift,
                output,
            );
        }
        Value::Double(value) => {
            return float(
                value.to_float(),
                floating::DOUBLE_DIGITS,
                places,
                shift,
                output,
            );
        }
        _ => {
            let decimal = from_value(value, FloatConversion::Retained)?;
            let rounded = decimal.round_dp_with_strategy(
                (u32::from(places) + shift).min(MAX_STORED_SCALE),
                RoundingStrategy::MidpointAwayFromZero,
            );
            (
                rounded.mantissa().unsigned_abs(),
                rounded.scale() as i32,
                rounded.is_sign_negative(),
            )
        }
    };
    finish(coefficient, shift as i32 - scale, negative)
}
fn float(
    value: f64,
    digits: usize,
    places: u8,
    shift: u32,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    let parts = floating::parts(value, digits, output)?;
    let exponent = parts.exponent + shift as i32;
    let drop = -(exponent + i32::from(places));
    let coefficient = if drop > digits as i32 {
        0
    } else if drop > 0 {
        let divisor = Decimal::from(DECIMAL_RADIX.pow(drop as u32));
        Decimal::from(parts.coefficient)
            .checked_div(divisor)
            .ok_or(EvalError::Overflow)?
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .checked_mul(divisor)
            .ok_or(EvalError::Overflow)?
            .mantissa()
            .unsigned_abs()
    } else {
        u128::from(parts.coefficient)
    };
    finish(coefficient, exponent, value.is_sign_negative())
}
fn finish(coefficient: u128, exponent: i32, negative: bool) -> Result<Magnitude, EvalError> {
    let mut text = FormatBuffer::new();
    write!(text, "{coefficient}").map_err(|_| EvalError::TextLimit)?;
    Ok(Magnitude {
        coefficient,
        point: text.text()?.len() as i32 + exponent,
        negative: negative && coefficient != 0,
    })
}
