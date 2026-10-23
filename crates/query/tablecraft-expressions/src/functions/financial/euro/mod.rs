//! Fixed euro conversions over the documented Double call envelope.
mod rates;

use super::super::conversions::Conversion;
use super::FinancialContext;
use crate::EvalError;
use rates::Currency;
use tablecraft_core::values::{Double, Value};

const DECIMAL_RADIX: f64 = 10.0;
const MIN_TRIANGULATION_DIGITS: i16 = 3;
// Seventeen significant decimal digits already round-trip every finite Double.
const ROUNDTRIP_DIGITS: i16 = 17;
const MAX_DECIMAL_POWER: i32 = f64::MAX_10_EXP;
const EXACT_INTEGER_BOUND: f64 = (1_u64 << f64::MANTISSA_DIGITS) as f64;

pub(super) fn apply_with_context(
    arguments: &[Value],
    context: &mut impl FinancialContext,
) -> Result<Value, EvalError> {
    context.step()?;
    let [number, source, target, options @ ..] = arguments else {
        return Err(EvalError::UnsupportedExpression);
    };
    let Value::Double(amount) = context.convert(Conversion::Double, number)? else {
        return Err(EvalError::TypeMismatch);
    };
    let source = Currency::parse(source, &mut || context.step())?;
    let target = Currency::parse(target, &mut || context.step())?;
    let full = match options.first() {
        Some(value) => {
            let Value::Boolean(value) = context.convert(Conversion::Boolean, value)? else {
                return Err(EvalError::TypeMismatch);
            };
            value
        }
        None => false,
    };
    let precision = match options.get(1) {
        Some(value) => {
            let Value::Int16(value) = context.convert(Conversion::Integer, value)? else {
                return Err(EvalError::TypeMismatch);
            };
            if value < MIN_TRIANGULATION_DIGITS {
                return Err(EvalError::Domain);
            }
            Some(value)
        }
        None => None,
    };
    context.step()?;
    let amount = amount.to_float();
    if source == target {
        return Ok(Value::Double(Double::from_float(amount)));
    }

    let mut euro = amount / source.rate();
    finite(euro)?;
    if full && euro == 0.0 && amount != 0.0 {
        return Err(EvalError::PrecisionLoss);
    }
    if !source.is_euro()
        && !target.is_euro()
        && let Some(precision) = precision
    {
        euro = significant(euro, precision)?;
    }
    context.step()?;
    let converted = euro * target.rate();
    finite(converted)?;
    if full && converted == 0.0 && euro != 0.0 {
        return Err(EvalError::PrecisionLoss);
    }
    let result = if full {
        converted
    } else {
        places(converted, target.places)?
    };
    context.step()?;
    Ok(Value::Double(Double::from_float(result)))
}

fn significant(value: f64, digits: i16) -> Result<f64, EvalError> {
    if value == 0.0 || digits >= ROUNDTRIP_DIGITS {
        return Ok(value);
    }
    let exponent = value.abs().log10().floor() as i32;
    let power = i32::from(digits) - 1 - exponent;
    let result = if power > MAX_DECIMAL_POWER {
        let tail = DECIMAL_RADIX.powi(power - MAX_DECIMAL_POWER);
        let head = DECIMAL_RADIX.powi(MAX_DECIMAL_POWER);
        ((value * head * tail).round() / tail) / head
    } else {
        let factor = DECIMAL_RADIX.powi(power);
        (value * factor).round() / factor
    };
    finite(result)
}

fn places(value: f64, places: i32) -> Result<f64, EvalError> {
    let factor = DECIMAL_RADIX.powi(places);
    if value.abs() >= EXACT_INTEGER_BOUND / factor {
        return Ok(value);
    }
    finite((value * factor).round() / factor)
}

fn finite(value: f64) -> Result<f64, EvalError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(EvalError::Overflow)
    }
}

#[cfg(test)]
mod tests;
