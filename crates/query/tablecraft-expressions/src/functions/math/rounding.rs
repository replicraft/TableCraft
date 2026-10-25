use super::{MathContext, MathFunction};
use crate::{
    EvalError,
    functions::conversions::Conversion,
    numeric::decimal::{FloatConversion, from_value, into_value},
};
use rust_decimal::{Decimal as LibraryDecimal, RoundingStrategy, prelude::ToPrimitive};
use tablecraft_core::values::{Currency, Double, Single, Value};

mod precision;

// At this magnitude every finite binary64 value is already integral. Avoid
// changing large integers through an unnecessary multiply/divide round trip.
const DOUBLE_INTEGRAL_MAGNITUDE: f64 = (1_u64 << (f64::MANTISSA_DIGITS - 1)) as f64;
const CURRENCY_MULTIPLIER: i64 = 10_i64.pow(Currency::SCALE as u32);
// Preserve the existing multiply/round/divide profile in its original range.
const MAX_SCALED_FLOAT_PLACES: u32 = 28;

pub(super) fn apply_with_context<'a>(
    count: usize,
    context: &mut impl MathContext<'a>,
) -> Result<Value, EvalError> {
    let places = if count == MathFunction::ROUND_ARGUMENTS {
        Some(context.argument(1)?)
    } else {
        None
    };
    let places = checked_places(places.as_deref(), &mut |target, value| {
        context.convert(target, value)
    })?;
    let value = context.argument(0)?;
    let converted_value = if matches!(value.as_ref(), Value::Text(_)) {
        Some(context.convert(Conversion::Double, &value)?)
    } else {
        None
    };
    rounded(converted_value.as_ref().unwrap_or(&value), places)
}

fn checked_places(
    places: Option<&Value>,
    convert: &mut impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
) -> Result<u32, EvalError> {
    Ok(if let Some(places) = places {
        let Value::Int32(places) = convert(Conversion::Long, places)? else {
            return Err(EvalError::TypeMismatch);
        };
        if places < 0 {
            return Err(EvalError::Domain);
        }
        places as u32
    } else {
        0
    })
}

fn rounded(value: &Value, places: u32) -> Result<Value, EvalError> {
    Ok(match value {
        Value::Null => Value::Null,
        Value::Boolean(value) => Value::Int16(if *value { -1 } else { 0 }),
        Value::Byte(value) => Value::Byte(*value),
        Value::Int16(value) => Value::Int16(*value),
        Value::Int32(value) => Value::Int32(*value),
        Value::Int64(value) => Value::Int64(*value),
        Value::Decimal(_) => {
            return into_value(
                from_value(value, FloatConversion::Retained)?
                    .round_dp_with_strategy(places, RoundingStrategy::MidpointNearestEven),
            );
        }
        Value::Currency(_) => {
            let rounded = from_value(value, FloatConversion::Retained)?
                .round_dp_with_strategy(places, RoundingStrategy::MidpointNearestEven);
            let scaled = rounded
                .checked_mul(LibraryDecimal::from(CURRENCY_MULTIPLIER))
                .ok_or(EvalError::Overflow)?;
            if !scaled.fract().is_zero() {
                return Err(EvalError::PrecisionLoss);
            }
            Value::Currency(Currency::from_scaled(
                scaled.to_i64().ok_or(EvalError::Overflow)?,
            ))
        }
        Value::Single(value) => Value::Single(Single::from_float(float(
            f64::from(value.to_float()),
            places,
        )? as f32)),
        Value::Double(value) => Value::Double(Double::from_float(float(value.to_float(), places)?)),
        _ => return Err(EvalError::TypeMismatch),
    })
}
fn float(value: f64, places: u32) -> Result<f64, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::NonFinite);
    }
    if value.abs() >= DOUBLE_INTEGRAL_MAGNITUDE {
        return Ok(value);
    }
    if places > MAX_SCALED_FLOAT_PLACES {
        return precision::round(value, places);
    }

    let factor = 10_f64.powi(places as i32);
    let magnitude = value.abs();
    if 1.0 / factor < magnitude - magnitude.next_down() {
        return Ok(value);
    }
    Ok((value * factor).round_ties_even() / factor)
}
