//! Explicit numeric coercion and checked scalar arithmetic, without text parsing.
use crate::EvalError;
use tablecraft_core::values::Value;

mod arithmetic;
mod coercion;
mod compare;
pub(crate) mod decimal;
mod promotion;
pub use arithmetic::{Arithmetic, arithmetic, negate};
pub use coercion::{IntegerType, coerce_integer};
pub(crate) use compare::compare;
pub use promotion::promoted_arithmetic;

pub(super) fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Boolean(value) => Some(if *value { -1 } else { 0 }),
        Value::Byte(value) => Some(i64::from(*value)),
        Value::Int16(value) => Some(i64::from(*value)),
        Value::Int32(value) => Some(i64::from(*value)),
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}
pub(super) fn fixed(value: &Value) -> Option<(i128, u8)> {
    match value {
        Value::Currency(value) => Some((
            i128::from(value.scaled()),
            tablecraft_core::values::Currency::SCALE,
        )),
        Value::Decimal(value) => Some((value.coefficient(), value.scale())),
        _ => integer(value).map(|n| (i128::from(n), 0)),
    }
}
pub(super) fn float(value: &Value) -> Result<f64, EvalError> {
    let result = match value {
        Value::Single(value) => f64::from(value.to_float()),
        Value::Double(value) => value.to_float(),
        Value::Currency(_) | Value::Decimal(_) => {
            return Err(EvalError::UnsupportedNumericCombination);
        }
        _ => {
            let n = integer(value).ok_or(EvalError::TypeMismatch)?;
            let result = n as f64;
            if result as i128 != i128::from(n) {
                return Err(EvalError::PrecisionLoss);
            }
            result
        }
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(EvalError::NonFinite)
    }
}
