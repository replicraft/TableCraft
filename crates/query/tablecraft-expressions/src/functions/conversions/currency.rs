//! Currency conversion through a bounded decimal library, behind Access carriers.
use crate::EvalError;
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use tablecraft_core::values::{Currency, Value};

const CURRENCY_SCALE: u32 = Currency::SCALE as u32;
const CURRENCY_MULTIPLIER: i64 = 10_i64.pow(CURRENCY_SCALE);

pub(super) fn convert(value: &Value) -> Result<Value, EvalError> {
    let decimal = match value {
        Value::Currency(value) => return Ok(Value::Currency(*value)),
        Value::Boolean(value) => {
            if *value {
                Decimal::NEGATIVE_ONE
            } else {
                Decimal::ZERO
            }
        }
        Value::Byte(value) => Decimal::from(*value),
        Value::Int16(value) => Decimal::from(*value),
        Value::Int32(value) => Decimal::from(*value),
        Value::Int64(value) => Decimal::from(*value),
        Value::Decimal(value) => {
            Decimal::try_from_i128_with_scale(value.coefficient(), u32::from(value.scale()))
                .map_err(|_| EvalError::Overflow)?
        }
        Value::Single(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            Decimal::from_f32_retain(value).ok_or(EvalError::Overflow)?
        }
        Value::Double(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            Decimal::from_f64_retain(value).ok_or(EvalError::Overflow)?
        }
        _ => return Err(EvalError::TypeMismatch),
    };
    let rounded =
        decimal.round_dp_with_strategy(CURRENCY_SCALE, RoundingStrategy::MidpointNearestEven);
    let scaled = rounded
        .checked_mul(Decimal::from(CURRENCY_MULTIPLIER))
        .ok_or(EvalError::Overflow)?;
    // Do not rely on truncating ToPrimitive behavior if this invariant changes.
    if !scaled.fract().is_zero() {
        return Err(EvalError::PrecisionLoss);
    }
    let scaled = scaled.to_i64().ok_or(EvalError::Overflow)?;
    Ok(Value::Currency(Currency::from_scaled(scaled)))
}
#[cfg(test)]
mod tests;
