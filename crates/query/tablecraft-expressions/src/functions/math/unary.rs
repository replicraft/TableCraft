use super::MathFunction;
use crate::{EvalError, numeric};
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single, Value};

pub(super) fn apply(function: MathFunction, value: &Value) -> Result<Value, EvalError> {
    if matches!(value, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(function, MathFunction::Sgn) {
        let sign = if let Some((coefficient, _)) = numeric::fixed(value) {
            coefficient.signum() as i16
        } else {
            let number = match value {
                Value::DateTime(date) => date.ole_days(),
                _ => numeric::float(value)?,
            };
            if number == 0.0 {
                0
            } else if number < 0.0 {
                -1
            } else {
                1
            }
        };
        return Ok(Value::Int16(sign));
    }
    let abs = matches!(function, MathFunction::Abs);
    Ok(match value {
        Value::Boolean(value) => Value::Int16(if *value { if abs { 1 } else { -1 } } else { 0 }),
        Value::Byte(value) => Value::Byte(*value),
        Value::Int16(value) => Value::Int16(if abs {
            value.checked_abs().ok_or(EvalError::Overflow)?
        } else {
            *value
        }),
        Value::Int32(value) => Value::Int32(if abs {
            value.checked_abs().ok_or(EvalError::Overflow)?
        } else {
            *value
        }),
        Value::Int64(value) => Value::Int64(if abs {
            value.checked_abs().ok_or(EvalError::Overflow)?
        } else {
            *value
        }),
        Value::Single(value) => Value::Single(Single::from_float(float(
            function,
            f64::from(value.to_float()),
        )? as f32)),
        Value::Double(value) => {
            Value::Double(Double::from_float(float(function, value.to_float())?))
        }
        Value::DateTime(value) => {
            let number = float(function, value.ole_days())?;
            if abs {
                Value::Double(Double::from_float(number))
            } else {
                Value::DateTime(
                    LegacyDateTime::from_ole_days(number).map_err(|_| EvalError::Overflow)?,
                )
            }
        }
        Value::Currency(value) => {
            let coefficient = value.scaled();
            let scale = 10_i64.pow(u32::from(Currency::SCALE));
            let result = if abs {
                coefficient.checked_abs()
            } else {
                let whole = if matches!(function, MathFunction::Int) {
                    coefficient.div_euclid(scale)
                } else {
                    coefficient / scale
                };
                whole.checked_mul(scale)
            }
            .ok_or(EvalError::Overflow)?;
            Value::Currency(Currency::from_scaled(result))
        }
        Value::Decimal(value) => {
            let coefficient = value.coefficient();
            if abs {
                Value::Decimal(
                    Decimal::new(coefficient.abs(), value.scale())
                        .map_err(|_| EvalError::Overflow)?,
                )
            } else {
                let divisor = 10_i128.pow(u32::from(value.scale()));
                let whole = if matches!(function, MathFunction::Int) {
                    coefficient.div_euclid(divisor)
                } else {
                    coefficient / divisor
                };
                Value::Decimal(Decimal::new(whole, 0).map_err(|_| EvalError::Overflow)?)
            }
        }
        _ => return Err(EvalError::TypeMismatch),
    })
}
fn float(function: MathFunction, value: f64) -> Result<f64, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::NonFinite);
    }
    Ok(match function {
        MathFunction::Abs => value.abs(),
        MathFunction::Int => value.floor(),
        MathFunction::Fix => value.trunc(),
        _ => return Err(EvalError::UnsupportedExpression),
    })
}
