//! Currency conversion through a bounded decimal library, behind Access carriers.
use crate::{
    EvalError,
    numeric::decimal::{FloatConversion, from_value},
};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use tablecraft_core::values::{Currency, Value};

const CURRENCY_SCALE: u32 = Currency::SCALE as u32;
const CURRENCY_MULTIPLIER: i64 = 10_i64.pow(CURRENCY_SCALE);

pub(super) fn convert(value: &Value) -> Result<Value, EvalError> {
    if let Value::Currency(value) = value {
        return Ok(Value::Currency(*value));
    }
    let decimal = from_value(value, FloatConversion::Retained)?;
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
