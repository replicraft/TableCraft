//! Empty selects an operator-specific zero without erasing its counterpart type.
use super::Evaluator;
use crate::{
    EvalError,
    functions::conversions::{Conversion, errors::into_operator_scalar},
};
use tablecraft_core::values::{Double, ExpressionValue, Value};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

/// Admit eager arithmetic data. Empty selects a zero that preserves the existing
/// effective-type rules; ordinary scalars retain their original carriers. This
/// neither parses Text, determines declarations nor makes Empty a stored value.
pub fn arithmetic_operands(
    operator: B,
    left: ExpressionValue,
    right: ExpressionValue,
) -> Result<(Value, Value), EvalError> {
    if !matches!(
        operator,
        B::Add | B::Subtract | B::Multiply | B::Divide | B::IntegerDivide | B::Modulo | B::Power
    ) {
        return Err(EvalError::UnsupportedExpression);
    }
    if operator == B::Divide && empty_division_operand(&left, &right).is_some() {
        // A plain zero would erase the original-type exception for 0/Empty.
        // The owning evaluator must validate the numerator under its policies.
        return Err(EvalError::UnsupportedExpression);
    }

    let left = admit(left)?;
    let right = admit(right)?;

    Ok(match (left, right) {
        (Some(left), Some(right)) => (left, right),
        (None, Some(right)) => (zero(operator, &right), right),
        (Some(left), None) => {
            let right = zero(operator, &left);
            (left, right)
        }
        (None, None) => (Value::Int16(0), Value::Int16(0)),
    })
}

/// Identify the original-type exception for division by Empty before lowering.
/// The returned numerator must use Evaluator::apply_empty_division so numeric
/// Text policies and cancellation remain visible before DivisionByZero.
pub fn empty_division_operand<'a>(
    left: &'a ExpressionValue,
    right: &ExpressionValue,
) -> Option<&'a Value> {
    if let ExpressionValue::Value(
        value @ (Value::Single(_) | Value::Double(_) | Value::DateTime(_) | Value::Text(_)),
    ) = left
        && matches!(right, ExpressionValue::Empty)
    {
        Some(value)
    } else {
        None
    }
}

impl Evaluator<'_> {
    /// Validate an original Single/Double/String/Date numerator divided by
    /// Empty. These admitted inputs raise DivisionByZero even for zero/Empty;
    /// conversion/finite/policy/resource failures remain authoritative first.
    pub fn apply_empty_division(&mut self, numerator: &Value) -> Result<Value, EvalError> {
        self.check(0)?;
        if !matches!(
            numerator,
            Value::Single(_) | Value::Double(_) | Value::Text(_) | Value::DateTime(_)
        ) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.apply_conversion(Conversion::Double, numerator)?;
        Err(EvalError::DivisionByZero)
    }
}

/// Admit unary numeric signs. Logical Not retains its separate admission rules.
pub fn arithmetic_unary_operand(operator: U, value: ExpressionValue) -> Result<Value, EvalError> {
    if !matches!(operator, U::Plus | U::Minus) {
        return Err(EvalError::UnsupportedExpression);
    }
    match value {
        ExpressionValue::Empty => Ok(Value::Int16(0)),
        value => into_operator_scalar(value),
    }
}

fn admit(value: ExpressionValue) -> Result<Option<Value>, EvalError> {
    match value {
        ExpressionValue::Empty => Ok(None),
        value => into_operator_scalar(value).map(Some),
    }
}

fn zero(operator: B, counterpart: &Value) -> Value {
    if operator == B::Add && matches!(counterpart, Value::Text(_)) {
        return Value::Text(String::new());
    }
    if matches!(operator, B::IntegerDivide | B::Modulo)
        && matches!(
            counterpart,
            Value::Single(_)
                | Value::Double(_)
                | Value::Currency(_)
                | Value::Decimal(_)
                | Value::DateTime(_)
                | Value::Text(_)
        )
    {
        return Value::Int32(0);
    }
    if matches!(operator, B::Add | B::Subtract | B::Multiply)
        && matches!(counterpart, Value::Byte(_))
    {
        return Value::Byte(0);
    }
    if matches!(counterpart, Value::Text(_)) {
        return Value::Double(Double::from_bits(0));
    }
    // Keep Empty distinct from Date: manufacturing Date(0) would incorrectly
    // trigger the Date-minus-Date Double exception rather than a Date result.
    Value::Int16(0)
}

#[cfg(test)]
mod tests;
