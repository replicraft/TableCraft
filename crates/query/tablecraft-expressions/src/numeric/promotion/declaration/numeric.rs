//! Numeric carrier admission and declaration-dependent selected-width overflow.
use super::{ArithmeticDeclarations, OperandDeclaration, retains_integral_carrier};
use crate::{
    EvalError,
    numeric::{
        Arithmetic, admission, integer,
        promotion::{
            doubles, integer_operand_conversion, integral_target, promoted_arithmetic,
            small_float_operand,
        },
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

/// Division-specific effective Double selection in the explicit declared profile.
/// MS-VBAL 5.6.9.3.5 overrides generic Decimal dominance for these peers.
/// This predicate performs no admission, conversion or declaration inference.
/// Text still requires the contextual evaluator and its explicit number policy.
pub fn uses_declared_double_division(operator: Arithmetic, left: &Value, right: &Value) -> bool {
    let peer = |value: &Value| {
        matches!(
            value,
            Value::Double(_) | Value::Currency(_) | Value::DateTime(_) | Value::Text(_)
        )
    };

    operator == Arithmetic::Divide
        && ((matches!(left, Value::Decimal(_)) && peer(right))
            || (matches!(right, Value::Decimal(_)) && peer(left)))
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
    let (result, retained) = evaluated(operator, left, right)?;
    if declarations.result() == OperandDeclaration::Scalar && !retained {
        return Err(EvalError::Overflow);
    }
    Ok(result)
}

/// Evaluate once and return a value only when the implemented Scalar and Variant
/// rules agree. None means a Scalar would overflow while a Variant would widen.
/// Shared validation/conversion/arithmetic failures remain errors, not ambiguity.
/// This does not establish either operand's declaration or native Access typing.
/// Text conversion and its budgets belong to the contextual evaluator.
pub fn arithmetic_agreement(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, EvalError> {
    let (result, retained) = evaluated(operator, left, right)?;
    Ok(retained.then_some(result))
}

fn evaluated(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<(Value, bool), EvalError> {
    let result = if uses_declared_double_division(operator, left, right) {
        for operand in [left, right] {
            admission::materialized(operand)?;
        }

        doubles(operator, left, right)?
    } else {
        promoted_arithmetic(operator, left, right)?
    };
    if matches!(result, Value::Null) {
        return Ok((result, true));
    }

    let retained = if matches!(operator, Arithmetic::IntegerDivide | Arithmetic::Modulo) {
        retains_integral_carrier(integer_operand_conversion(left, right)?, &result)
    } else if integer(left).is_some()
        && integer(right).is_some()
        && !matches!(operator, Arithmetic::Divide | Arithmetic::Power)
    {
        retains_integral_carrier(integral_target(left, right), &result)
    } else if single_operation(operator, left, right) {
        matches!(result, Value::Single(_))
    } else if date_operation(operator, left, right) {
        matches!(result, Value::DateTime(_))
    } else {
        true
    };

    Ok((result, retained))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod agreement_tests;

#[cfg(test)]
mod division_tests;
