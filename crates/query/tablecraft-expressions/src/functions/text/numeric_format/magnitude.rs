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
const DECIMAL_RADIX: u128 = 10;
const HALF_DIVISOR: u128 = 2;

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
    prepare_shifted(value, places, shift as i32, output)
}

/// Display-only decimal-point scaling; it does not narrow the stored carrier.
pub(in crate::functions::text) fn prepare_shifted(
    value: &Value,
    places: u8,
    shift: i32,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    let value = unrounded(value, shift, output)?;
    let exponent = value.point - digits(value.coefficient)?;
    round(value.coefficient, exponent, value.negative, places, output)
}

/// Scientific normalization must retain even finite subnormals beyond 255 places.
pub(in crate::functions::text) fn unrounded(
    value: &Value,
    shift: i32,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    let floating = match value {
        Value::Single(value) => Some((f64::from(value.to_float()), floating::SINGLE_DIGITS)),
        Value::Double(value) => Some((value.to_float(), floating::DOUBLE_DIGITS)),
        _ => None,
    };
    if let Some((value, digits)) = floating {
        let parts = floating::parts(value, digits, output)?;
        return finish(
            u128::from(parts.coefficient),
            parts.exponent + shift,
            value.is_sign_negative(),
        );
    }
    let decimal = from_value(value, FloatConversion::Retained)?;
    finish(
        decimal.mantissa().unsigned_abs(),
        shift - decimal.scale() as i32,
        decimal.is_sign_negative(),
    )
}

fn round(
    coefficient: u128,
    exponent: i32,
    negative: bool,
    places: u8,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    output.step()?;
    let drop = -(exponent + i32::from(places));
    let rounded = if drop > MAX_STORED_SCALE as i32 + 1 {
        0
    } else if drop > 0 {
        let divisor = DECIMAL_RADIX.pow(drop as u32);
        let integral = if drop as u32 > MAX_STORED_SCALE {
            // A 96-bit coefficient has at most 29 digits. At this last position
            // the result is zero or one; scale 29 cannot be represented by Decimal.
            u128::from(coefficient >= divisor / HALF_DIVISOR)
        } else {
            Decimal::try_from_i128_with_scale(
                i128::try_from(coefficient).map_err(|_| EvalError::Overflow)?,
                drop as u32,
            )
            .map_err(|_| EvalError::Overflow)?
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .mantissa()
            .unsigned_abs()
        };
        integral * divisor
    } else {
        coefficient
    };
    finish(rounded, exponent, negative)
}

/// Round a normalized display mantissa, retaining its independent exponent.
pub(in crate::functions::text) fn round_mantissa(
    value: Magnitude,
    whole: i32,
    places: u8,
    output: &mut impl TextOutput,
) -> Result<Magnitude, EvalError> {
    round(
        value.coefficient,
        whole - digits(value.coefficient)?,
        value.negative,
        places,
        output,
    )
}

fn digits(coefficient: u128) -> Result<i32, EvalError> {
    let mut text = FormatBuffer::new();
    write!(text, "{coefficient}").map_err(|_| EvalError::TextLimit)?;
    Ok(text.text()?.len() as i32)
}

fn finish(coefficient: u128, exponent: i32, negative: bool) -> Result<Magnitude, EvalError> {
    Ok(Magnitude {
        coefficient,
        point: digits(coefficient)? + exponent,
        negative: negative && coefficient != 0,
    })
}
