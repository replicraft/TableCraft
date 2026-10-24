//! Borrowed text-call admission keeps source and numeric Empty targets explicit.
use super::Evaluator;
use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

pub(super) fn scalar<'a>(
    value: &'a ExpressionValue,
    empty: &'a Value,
) -> Result<&'a Value, EvalError> {
    match value {
        ExpressionValue::Value(value) => Ok(value),
        ExpressionValue::Empty => Ok(empty),
        ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
        ExpressionValue::StringBytes(_) => Err(EvalError::NonScalarExpression),
        ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
    }
}

impl Evaluator<'_> {
    /// Call only after validating the owning function's bounded argument count.
    pub(super) fn text_expression_limits(
        &self,
        arguments: &[ExpressionValue],
    ) -> Result<(), EvalError> {
        for value in arguments {
            if matches!(value, ExpressionValue::Value(Value::Text(text)) if text.len() > self.limits.max_text_bytes)
            {
                return Err(EvalError::TextLimit);
            }
        }

        Ok(())
    }
}
