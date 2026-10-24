//! Numeric carrier admission and declaration-dependent selected-width overflow.
use super::{ArithmeticDeclarations, OperandDeclaration, declared_integral_arithmetic};
use crate::{
    EvalError,
    numeric::{
        Arithmetic, admission, integer,
        promotion::{integer_operand_conversion, promoted_arithmetic, small_float_operand},
    },
};
use tablecraft_core::values::Value;

fn numeric_operand(value: &Value) -> bool {
    integer(value).is_some()
        || matches!(
            value,
            Value::Single(_) | Value::Double(_) | Value::Currency(_) | Value::Decimal(_)
        )
}

fn rounded_operand(value: &Value) -> bool {
    numeric_operand(value) || matches!(value, Value::DateTime(_) | Value::Text(_))
}

fn single_operation(operator: Arithmetic, left: &Value, right: &Value) -> bool {
    matches!(
        operator,
        Arithmetic::Add | Arithmetic::Subtract | Arithmetic::Multiply | Arithmetic::Divide
    ) && (matches!(left, Value::Single(_)) || matches!(right, Value::Single(_)))
        && small_float_operand(left)
        && small_float_operand(right)
}

fn date_operation(operator: Arithmetic, left: &Value, right: &Value) -> bool {
    let date = |value: &Value| matches!(value, Value::DateTime(_));
    matches!(operator, Arithmetic::Add | Arithmetic::Subtract)
        && (date(left) || date(right))
        && !matches!(left, Value::Decimal(_))
        && !matches!(right, Value::Decimal(_))
        && (date(left) || numeric_operand(left) || matches!(left, Value::Text(_)))
        && (date(right) || numeric_operand(right) || matches!(right, Value::Text(_)))
        && !(operator == Arithmetic::Subtract && date(left) && date(right))
}

/// Whether the current materialized arithmetic profile enforces an integral or
/// Single/Date width using declaration provenance. This is a dispatch predicate, not
/// validation or native SQL type inference. NULL and Double-selected operations
/// do not require a declaration-dependent width decision. Neither do remainders,
/// fixed LongLong results or Byte/Byte quotients. Rounded numeric inputs
/// participate after checked conversion. Date/Text pairs require the evaluator
/// contextual conversion service; the standalone kernel still rejects Text.
pub fn uses_declared_overflow(operator: Arithmetic, left: &Value, right: &Value) -> bool {
    if operator == Arithmetic::Modulo {
        // A remainder always fits its converted operand width. Range checking
        // and zero-divisor errors do not depend on declaration categories.
        return false;
    }

    let long_long = matches!(left, Value::Int64(_)) || matches!(right, Value::Int64(_));
    if operator == Arithmetic::IntegerDivide {
        // LongLong cannot widen, and unsigned Byte/Byte cannot overflow its
        // quotient carrier. Other widths retain their MIN / -1 decision.
        return rounded_operand(left)
            && rounded_operand(right)
            && !long_long
            && !matches!((left, right), (Value::Byte(_), Value::Byte(_)));
    }

    (integer(left).is_some()
        && integer(right).is_some()
        && !long_long
        && !matches!(operator, Arithmetic::Divide | Arithmetic::Power))
        || single_operation(operator, left, right)
        || date_operation(operator, left, right)
}

/// Apply the supported numeric promotion profile with original Scalar/Variant
/// categories. Two Scalars prohibit widening beyond selected integral/Single
/// widths or the legacy Date envelope; a Variant participant retains materialized
/// widening. Date-minus-Date remains Double. Currency,
/// Decimal, LongLong and Double keep their existing checked range rules.
///
/// Inputs must be nullable ordinary numeric or legacy Date carriers. Family
/// validation precedes NULL propagation. Text belongs to contextual conversion services.
/// This bounded kernel allocates nothing and does not infer ACE SQL declarations.
pub fn declared_numeric_arithmetic(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
    declarations: ArithmeticDeclarations,
) -> Result<Value, EvalError> {
    for value in [left, right] {
        admission::materialized(value)?;
    }
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if integer(left).is_some() && integer(right).is_some() {
        return declared_integral_arithmetic(operator, left, right, declarations);
    }
    if matches!(operator, Arithmetic::IntegerDivide | Arithmetic::Modulo) {
        let target = integer_operand_conversion(left, right)?;
        let left = target.apply(left)?;
        let right = target.apply(right)?;
        return declared_integral_arithmetic(operator, &left, &right, declarations);
    }
    let result = promoted_arithmetic(operator, left, right)?;
    let widened_single =
        single_operation(operator, left, right) && !matches!(result, Value::Single(_));
    let widened_date =
        date_operation(operator, left, right) && !matches!(result, Value::DateTime(_));
    if declarations.result() == OperandDeclaration::Scalar && (widened_single || widened_date) {
        return Err(EvalError::Overflow);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
