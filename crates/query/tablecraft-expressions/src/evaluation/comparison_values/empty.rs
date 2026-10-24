//! Host Empty selects zero or empty String without changing stored values.
use super::Evaluator;
use crate::{EvalError, comparison::Comparison, logic::Truth};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    pub(super) fn compare_empty_values(
        &mut self,
        operator: Comparison,
        left: &ExpressionValue,
        right: &ExpressionValue,
    ) -> Result<Option<Truth>, EvalError> {
        if !matches!(left, ExpressionValue::Empty) && !matches!(right, ExpressionValue::Empty) {
            return Ok(None);
        }

        self.check(0)?;
        let converted_left = if matches!(left, ExpressionValue::Empty) {
            Some(empty_operand(right)?)
        } else {
            None
        };
        let converted_right = if matches!(right, ExpressionValue::Empty) {
            Some(empty_operand(left)?)
        } else {
            None
        };

        // Neither borrowed operand is Empty after admission. Reuse the owning
        // numeric/date/String services, including raw-unit provider selection.
        self.compare_expression_values(
            operator,
            converted_left.as_ref().unwrap_or(left),
            converted_right.as_ref().unwrap_or(right),
        )
        .map(Some)
    }
}

fn empty_operand(counterpart: &ExpressionValue) -> Result<ExpressionValue, EvalError> {
    let value = match counterpart {
        ExpressionValue::StringBytes(_) | ExpressionValue::Value(Value::Text(_)) => {
            Value::Text(String::new())
        }
        ExpressionValue::Value(Value::Byte(_)) => Value::Byte(0),
        ExpressionValue::Empty
        | ExpressionValue::Value(
            Value::Null
            | Value::Boolean(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_),
        ) => Value::Int16(0),
        ExpressionValue::Error(_) => return Err(EvalError::TypeMismatch),
        _ => return Err(EvalError::NonScalarExpression),
    };

    Ok(ExpressionValue::Value(value))
}

#[cfg(test)]
mod tests;
