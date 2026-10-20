//! Explicit IEEE conversion, without nullable/implicit arithmetic coercion.
use crate::EvalError;
use std::fmt::{self, Write};
use tablecraft_core::values::{Decimal, Double, Single, Value};

// Sign + leading zero + decimal point cover the longest scale-28 spelling.
// Currency's signed 19-digit coefficient also fits this existing field envelope.
const FIXED_LITERAL_BYTES: usize = Decimal::MAX_PRECISION as usize + 3;

pub(super) fn single(value: &Value) -> Result<Value, EvalError> {
    let converted = match value {
        Value::Boolean(value) => {
            if *value {
                -1.0_f32
            } else {
                0.0_f32
            }
        }
        Value::Byte(value) => f32::from(*value),
        Value::Int16(value) => f32::from(*value),
        Value::Int32(value) => *value as f32,
        Value::Int64(value) => *value as f32,
        Value::Single(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            value
        }
        Value::Double(value) => {
            let value = value.to_float();
            if !value.is_finite() {
                return Err(EvalError::NonFinite);
            }
            if value.abs() > f64::from(f32::MAX) {
                return Err(EvalError::Overflow);
            }
            // This explicit cast may round, including gradual underflow to a
            // signed subnormal/zero. No implicit arithmetic policy is changed.
            value as f32
        }
        Value::Currency(value) => literal(value)?
            .as_str()?
            .parse::<f32>()
            .map_err(|_| EvalError::UnsupportedNumericCombination)?,
        Value::Decimal(value) => literal(value)?
            .as_str()?
            .parse::<f32>()
            .map_err(|_| EvalError::UnsupportedNumericCombination)?,
        _ => return Err(EvalError::TypeMismatch),
    };
    if !converted.is_finite() {
        return Err(EvalError::Overflow);
    }
    Ok(Value::Single(Single::from_float(converted)))
}
pub(super) fn double(value: &Value) -> Result<Value, EvalError> {
    let converted = match value {
        Value::Boolean(value) => {
            if *value {
                -1.0_f64
            } else {
                0.0_f64
            }
        }
        Value::Byte(value) => f64::from(*value),
        Value::Int16(value) => f64::from(*value),
        Value::Int32(value) => f64::from(*value),
        Value::Int64(value) => *value as f64,
        Value::Single(value) => f64::from(value.to_float()),
        Value::Double(value) => value.to_float(),
        Value::Currency(value) => literal(value)?
            .as_str()?
            .parse::<f64>()
            .map_err(|_| EvalError::UnsupportedNumericCombination)?,
        Value::Decimal(value) => literal(value)?
            .as_str()?
            .parse::<f64>()
            .map_err(|_| EvalError::UnsupportedNumericCombination)?,
        _ => return Err(EvalError::TypeMismatch),
    };
    if !converted.is_finite() {
        return Err(EvalError::NonFinite);
    }
    Ok(Value::Double(Double::from_float(converted)))
}
struct FixedLiteral {
    bytes: [u8; FIXED_LITERAL_BYTES],
    length: usize,
}
impl Write for FixedLiteral {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
impl FixedLiteral {
    fn as_str(&self) -> Result<&str, EvalError> {
        std::str::from_utf8(&self.bytes[..self.length])
            .map_err(|_| EvalError::UnsupportedNumericCombination)
    }
}
fn literal(value: &impl fmt::Display) -> Result<FixedLiteral, EvalError> {
    let mut literal = FixedLiteral {
        bytes: [0; FIXED_LITERAL_BYTES],
        length: 0,
    };
    write!(&mut literal, "{value}").map_err(|_| EvalError::UnsupportedNumericCombination)?;
    Ok(literal)
}
#[cfg(test)]
mod tests;
