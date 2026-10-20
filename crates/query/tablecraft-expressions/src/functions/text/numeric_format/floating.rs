//! Explicit finite Single/Double spelling with bounded significant-digit rounding.
use super::{Literal, TextOutput};
use crate::EvalError;
use std::fmt::Write;

pub(super) const SINGLE_DIGITS: usize = 7;
pub(super) const DOUBLE_DIGITS: usize = 15;

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
    let mut scientific = Literal::new();
    let places = digits - 1;
    write!(scientific, "{:.places$e}", value.abs()).map_err(|_| EvalError::TextLimit)?;
    let (mantissa, exponent) = scientific
        .text()?
        .split_once('e')
        .ok_or(EvalError::Domain)?;
    let exponent: i32 = exponent.parse().map_err(|_| EvalError::Domain)?;
    let mut significand = [0; DOUBLE_DIGITS];
    let mut length = 0;
    for byte in mantissa.bytes().filter(|byte| *byte != b'.') {
        output.step()?;
        *significand.get_mut(length).ok_or(EvalError::TextLimit)? = byte;
        length += 1;
    }
    while length > 1 && significand[length - 1] == b'0' {
        length -= 1;
    }
    let significand =
        std::str::from_utf8(&significand[..length]).map_err(|_| EvalError::UnrepresentableText)?;
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
