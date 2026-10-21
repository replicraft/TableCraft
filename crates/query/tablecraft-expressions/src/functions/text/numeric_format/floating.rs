//! Explicit finite Single/Double spelling with bounded significant-digit rounding.
use super::{Literal, TextOutput};
use crate::EvalError;
use std::fmt::Write;

pub(in crate::functions::text) const SINGLE_DIGITS: usize = 7;
pub(in crate::functions::text) const DOUBLE_DIGITS: usize = 15;

pub(super) fn write(
    value: f64,
    digits: usize,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    if !value.is_finite() {
        return Err(EvalError::NonFinite);
    }
    if value == 0.0 {
        return output.append(" 0");
    }
    let parts = parts(value, digits, output)?;
    let exponent = parts.exponent + digits as i32 - 1;
    let mut text = Literal::new();
    write!(text, "{}", parts.coefficient).map_err(|_| EvalError::TextLimit)?;
    let significand = text.text()?.trim_end_matches('0');
    let length = significand.len();
    output.append(if value < 0.0 { "-" } else { " " })?;
    if exponent >= digits as i32 {
        output.append(&significand[..1])?;
        if length > 1 {
            output.append(".")?;
            output.append(&significand[1..])?;
        }
        let mut suffix = Literal::new();
        write!(suffix, "E{exponent:+}").map_err(|_| EvalError::TextLimit)?;
        return output.append(suffix.text()?);
    }
    let whole = exponent + 1;
    if whole <= 0 {
        output.append("0.")?;
        for _ in whole..0 {
            output.append("0")?;
        }
        output.append(significand)
    } else {
        let whole = whole as usize;
        let split = whole.min(length);
        output.append(&significand[..split])?;
        for _ in length..whole {
            output.append("0")?;
        }
        if split < length {
            output.append(".")?;
            output.append(&significand[split..])?;
        }
        Ok(())
    }
}

/// Bounded normalized significant digits, with a signed base-ten exponent.
/// Keeping exponent separate supports every finite IEEE magnitude without narrowing.
pub(in crate::functions::text) struct Scientific {
    pub coefficient: u64,
    pub exponent: i32,
}
pub(in crate::functions::text) fn parts(
    value: f64,
    digits: usize,
    output: &mut impl TextOutput,
) -> Result<Scientific, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::NonFinite);
    }
    if value == 0.0 {
        return Ok(Scientific {
            coefficient: 0,
            exponent: 0,
        });
    }
    let mut scientific = Literal::new();
    let places = digits - 1;
    write!(scientific, "{:.places$e}", value.abs()).map_err(|_| EvalError::TextLimit)?;
    let (mantissa, exponent) = scientific
        .text()?
        .split_once('e')
        .ok_or(EvalError::Domain)?;
    let exponent: i32 = exponent.parse().map_err(|_| EvalError::Domain)?;
    let mut coefficient = 0_u64;
    for digit in mantissa.bytes().filter(|byte| *byte != b'.') {
        output.step()?;
        coefficient = coefficient
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(digit - b'0')))
            .ok_or(EvalError::Overflow)?;
    }
    Ok(Scientific {
        coefficient,
        exponent: exponent - places as i32,
    })
}
