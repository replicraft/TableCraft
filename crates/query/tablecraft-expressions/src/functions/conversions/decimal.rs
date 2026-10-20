//! CDec through the bounded arithmetic library, without field-range narrowing.
use crate::{
    EvalError,
    numeric::decimal::{FloatConversion, from_value},
};
use tablecraft_core::values::{Decimal, Value};

pub(super) fn convert(value: &Value) -> Result<Value, EvalError> {
    if let Value::Decimal(value) = value {
        return Ok(Value::Decimal(*value));
    }
    let decimal = from_value(value, FloatConversion::Normalized)?;
    let value =
        Decimal::new(decimal.mantissa(), decimal.scale() as u8).map_err(|_| EvalError::Overflow)?;
    Ok(Value::Decimal(value))
}
#[cfg(test)]
mod tests;
