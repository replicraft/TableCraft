//! Checked decimal operations; bounded library rounding is explicit policy.
use super::{FloatConversion, from_value, into_value};
use crate::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{self, Arithmetic},
};
use tablecraft_core::values::Value;

pub(in crate::numeric) fn apply(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<Value, EvalError> {
    if operator == Arithmetic::Power {
        // Exponentiation has a Double result, including Decimal operands.
        let left = Conversion::Double.apply(left)?;
        let right = Conversion::Double.apply(right)?;
        return numeric::arithmetic(operator, &left, &right);
    }
    let left = from_value(left, FloatConversion::Normalized)?;
    let right = from_value(right, FloatConversion::Normalized)?;
    let result = match operator {
        Arithmetic::Add => left.checked_add(right),
        Arithmetic::Subtract => left.checked_sub(right),
        Arithmetic::Multiply => left.checked_mul(right),
        Arithmetic::Divide => {
            if right.is_zero() {
                return Err(EvalError::DivisionByZero);
            }
            left.checked_div(right)
        }
        _ => return Err(EvalError::UnsupportedNumericCombination),
    }
    .ok_or(EvalError::Overflow)?;
    into_value(result)
}
#[cfg(test)]
mod tests;
