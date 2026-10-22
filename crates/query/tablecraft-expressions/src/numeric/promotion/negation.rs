//! Materialized unary minus, with independent checked kernel contracts retained.
use super::integral_result;
use crate::{
    EvalError,
    functions::{conversions::Conversion, dates::date_arithmetic},
    numeric::{self, Arithmetic},
};
use tablecraft_core::values::{Single, Value};

/// Negate a materialized numeric carrier. Byte/Boolean select Integer; Integer
/// and Long may widen, while LongLong/Currency overflow remains an error.
/// Text conversion belongs to the contextual evaluator; DATE uses raw serials.
pub fn promoted_negate(value: &Value) -> Result<Value, EvalError> {
    match value {
        Value::Byte(value) => integral_result(-i128::from(*value), Conversion::Integer),
        Value::Boolean(value) => Ok(Value::Int16(i16::from(*value))),
        Value::Int16(value) => integral_result(-i128::from(*value), Conversion::Integer),
        Value::Int32(value) => integral_result(-i128::from(*value), Conversion::Long),
        Value::Int64(value) => integral_result(-i128::from(*value), Conversion::LongLong),
        Value::Single(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            Ok(Value::Single(Single::from_float(-value)))
        }
        Value::DateTime(_) => date_arithmetic(Arithmetic::Subtract, &Value::Int16(0), value),
        _ => numeric::negate(value),
    }
}
