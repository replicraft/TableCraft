//! Private arithmetic-library boundary; domain carriers and schema stay independent.
use crate::EvalError;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use tablecraft_core::values::{Currency, Value};

pub(crate) enum FloatConversion {
    Retained,
    Normalized,
}

pub(crate) fn from_value(value: &Value, floats: FloatConversion) -> Result<Decimal, EvalError> {
    Ok(match value {
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
        Value::Currency(value) => Decimal::try_from_i128_with_scale(
            i128::from(value.scaled()),
            u32::from(Currency::SCALE),
        )
        .map_err(|_| EvalError::Overflow)?,
        Value::Decimal(value) => {
            Decimal::try_from_i128_with_scale(value.coefficient(), u32::from(value.scale()))
                .map_err(|_| EvalError::Overflow)?
        }
        Value::Single(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            match floats {
                FloatConversion::Retained => Decimal::from_f32_retain(value),
                FloatConversion::Normalized => Decimal::from_f32(value),
            }
            .ok_or(EvalError::Overflow)?
        }
        Value::Double(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            match floats {
                FloatConversion::Retained => Decimal::from_f64_retain(value),
                FloatConversion::Normalized => Decimal::from_f64(value),
            }
            .ok_or(EvalError::Overflow)?
        }
        _ => return Err(EvalError::TypeMismatch),
    })
}
