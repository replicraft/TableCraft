//! Operator-specific quotient, remainder and power selection from original carriers.
use super::{arithmetic, doubles, integral_result, single_result, small_float_operand};
use crate::{EvalError, functions::conversions::Conversion, numeric::Arithmetic};
use tablecraft_core::values::Value;

pub(super) fn divide(left: &Value, right: &Value) -> Result<Value, EvalError> {
    if matches!(left, Value::Decimal(_)) || matches!(right, Value::Decimal(_)) {
        let left = Conversion::Decimal.apply(left)?;
        let right = Conversion::Decimal.apply(right)?;
        return arithmetic(Arithmetic::Divide, &left, &right);
    }
    if matches!(
        left,
        Value::Currency(_) | Value::DateTime(_) | Value::Int64(_)
    ) || matches!(
        right,
        Value::Currency(_) | Value::DateTime(_) | Value::Int64(_)
    ) {
        return doubles(Arithmetic::Divide, left, right);
    }
    let result = arithmetic(Arithmetic::Divide, left, right)?;
    if (matches!(left, Value::Single(_)) || matches!(right, Value::Single(_)))
        && small_float_operand(left)
        && small_float_operand(right)
    {
        single_result(result)
    } else {
        Ok(result)
    }
}

pub(crate) fn integer_operand_conversion(
    left: &Value,
    right: &Value,
) -> Result<Conversion, EvalError> {
    let fractional = |value: &Value| {
        matches!(
            value,
            Value::Single(_)
                | Value::Double(_)
                | Value::Currency(_)
                | Value::Decimal(_)
                | Value::DateTime(_)
                | Value::Text(_)
        )
    };
    if matches!(left, Value::Int64(_)) || matches!(right, Value::Int64(_)) {
        return Ok(Conversion::LongLong);
    }
    if matches!(left, Value::Boolean(_) | Value::Int16(_)) && fractional(right) {
        return Ok(Conversion::Integer);
    }
    if fractional(left)
        || fractional(right)
        || matches!(left, Value::Int32(_))
        || matches!(right, Value::Int32(_))
    {
        return Ok(Conversion::Long);
    }
    match (left, right) {
        (Value::Byte(_), Value::Byte(_)) => Ok(Conversion::Byte),
        (
            Value::Byte(_) | Value::Boolean(_) | Value::Int16(_),
            Value::Byte(_) | Value::Boolean(_) | Value::Int16(_),
        ) => Ok(Conversion::Integer),
        _ => Err(EvalError::TypeMismatch),
    }
}

pub(super) fn rounded(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<Value, EvalError> {
    let target = integer_operand_conversion(left, right)?;
    let a = target.apply(left)?;
    let b = target.apply(right)?;
    let Value::Int64(value) = arithmetic(operator, &a, &b)? else {
        return Err(EvalError::TypeMismatch);
    };
    integral_result(i128::from(value), target)
}

pub(super) fn power(left: &Value, right: &Value) -> Result<Value, EvalError> {
    if matches!(
        left,
        Value::Currency(_) | Value::DateTime(_) | Value::Int64(_)
    ) || matches!(
        right,
        Value::Currency(_) | Value::DateTime(_) | Value::Int64(_)
    ) {
        doubles(Arithmetic::Power, left, right)
    } else {
        arithmetic(Arithmetic::Power, left, right)
    }
}
