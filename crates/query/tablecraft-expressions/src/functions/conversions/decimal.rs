//! CDec through the bounded arithmetic library, without field-range narrowing.
use crate::{
    EvalError,
    numeric::decimal::{FloatConversion, from_value, into_value},
};
use tablecraft_core::values::Value;

pub(super) fn convert(value: &Value) -> Result<Value, EvalError> {
    if let Value::Decimal(value) = value {
        return Ok(Value::Decimal(*value));
    }
    into_value(from_value(value, FloatConversion::Normalized)?)
}
#[cfg(test)]
mod tests;
