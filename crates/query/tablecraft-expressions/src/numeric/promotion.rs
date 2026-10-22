//! Materialized numeric value promotion, separate from the checked Int64 kernels.
use super::{Arithmetic, arithmetic, decimal, integer};
use crate::{
    EvalError,
    functions::{conversions::Conversion, dates::date_arithmetic},
};
use tablecraft_core::values::{Double, Single, Value};

/// Add, subtract or multiply materialized numeric carriers using an explicit
/// value-type ladder. This has no declared-expression typing information and
/// does not implement the static, non-Variant overflow branch. Text requires a
/// contextual evaluator. Integer-to-floating precision guards remain explicit.
pub fn promoted_arithmetic(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<Value, EvalError> {
    if !matches!(
        operator,
        Arithmetic::Add | Arithmetic::Subtract | Arithmetic::Multiply
    ) {
        return Err(EvalError::UnsupportedExpression);
    }
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(left, Value::Decimal(_)) || matches!(right, Value::Decimal(_)) {
        let left = Conversion::Decimal.apply(left)?;
        let right = Conversion::Decimal.apply(right)?;
        return arithmetic(operator, &left, &right);
    }
    if matches!(left, Value::DateTime(_)) || matches!(right, Value::DateTime(_)) {
        if operator != Arithmetic::Multiply {
            return date_arithmetic(operator, left, right);
        }
        return doubles(operator, left, right);
    }
    let currency = matches!(left, Value::Currency(_)) || matches!(right, Value::Currency(_));
    if currency {
        let floating = matches!(left, Value::Single(_) | Value::Double(_))
            || matches!(right, Value::Single(_) | Value::Double(_));
        if operator == Arithmetic::Multiply && floating {
            return doubles(operator, left, right);
        }
        let left = Conversion::Currency.apply(left)?;
        let right = Conversion::Currency.apply(right)?;
        if operator == Arithmetic::Multiply {
            // Valid Currency products fit Decimal exactly before the final
            // four-place half-even conversion. Larger products fail range checks.
            let product = decimal::apply(operator, &left, &right)?;
            return Conversion::Currency.apply(&product);
        }
        return arithmetic(operator, &left, &right);
    }
    if let (Some(a), Some(b)) = (integer(left), integer(right)) {
        let value = match operator {
            Arithmetic::Add => i128::from(a) + i128::from(b),
            Arithmetic::Subtract => i128::from(a) - i128::from(b),
            Arithmetic::Multiply => i128::from(a) * i128::from(b),
            _ => return Err(EvalError::UnsupportedExpression),
        };
        if matches!(left, Value::Int64(_)) || matches!(right, Value::Int64(_)) {
            return i64::try_from(value)
                .map(Value::Int64)
                .map_err(|_| EvalError::Overflow);
        }
        let long = matches!(left, Value::Int32(_)) || matches!(right, Value::Int32(_));
        let byte = matches!((left, right), (Value::Byte(_), Value::Byte(_)));
        if byte && let Ok(value) = u8::try_from(value) {
            return Ok(Value::Byte(value));
        }
        if !long && let Ok(value) = i16::try_from(value) {
            return Ok(Value::Int16(value));
        }
        if let Ok(value) = i32::try_from(value) {
            return Ok(Value::Int32(value));
        }
        // These operands are at most Int32, so their product is finite in f64.
        return Ok(Value::Double(Double::from_float(value as f64)));
    }
    let result = arithmetic(operator, left, right)?;
    let small = |value: &Value| {
        matches!(
            value,
            Value::Byte(_) | Value::Boolean(_) | Value::Int16(_) | Value::Single(_)
        )
    };
    if small(left) && small(right) {
        let Value::Double(value) = result else {
            return Err(EvalError::TypeMismatch);
        };
        let single = value.to_float() as f32;
        if single.is_finite() {
            return Ok(Value::Single(Single::from_float(single)));
        }
        return Ok(Value::Double(value));
    }
    Ok(result)
}

fn doubles(operator: Arithmetic, left: &Value, right: &Value) -> Result<Value, EvalError> {
    let left = Conversion::Double.apply(left)?;
    let right = Conversion::Double.apply(right)?;
    arithmetic(operator, &left, &right)
}

#[cfg(test)]
mod tests;
