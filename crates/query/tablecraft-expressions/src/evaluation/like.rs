//! LIKE admission for retained expression Strings.
use super::Evaluator;
use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply LIKE to retained expression Strings. A raw String paired with NULL
    /// needs the explicit wildcard mode, but no decoding or collation provider.
    /// Other raw operands retain strict Unicode publication before matching.
    pub fn apply_expression_like(
        &mut self,
        value: &ExpressionValue,
        pattern: &ExpressionValue,
        negated: bool,
    ) -> Result<Value, EvalError> {
        if matches!(
            (value, pattern),
            (
                ExpressionValue::StringBytes(_),
                ExpressionValue::Value(Value::Null)
            ) | (
                ExpressionValue::Value(Value::Null),
                ExpressionValue::StringBytes(_)
            )
        ) {
            self.check(0)?;
            self.like_mode.ok_or(EvalError::LikeModeRequired)?;
            return Ok(Value::Null);
        }

        let published_value = match value {
            ExpressionValue::StringBytes(bytes) => Some(self.publish_expression_string(bytes)?),
            _ => None,
        };
        let published_pattern = match pattern {
            ExpressionValue::StringBytes(bytes) => Some(self.publish_expression_string(bytes)?),
            _ => None,
        };
        let value = match (published_value.as_ref(), value) {
            (Some(value), _) | (None, ExpressionValue::Value(value)) => value,
            _ => return Err(EvalError::NonScalarExpression),
        };
        let pattern = match (published_pattern.as_ref(), pattern) {
            (Some(pattern), _) | (None, ExpressionValue::Value(pattern)) => pattern,
            _ => return Err(EvalError::NonScalarExpression),
        };

        self.apply_like(value, pattern, negated)
    }
}

#[cfg(test)]
mod tests;
