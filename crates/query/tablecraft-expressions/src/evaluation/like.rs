//! LIKE admission for retained expression Strings.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::LikeProfile,
    predicates::{LikeString, like_utf16},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply LIKE to retained expression Strings. A raw String paired with NULL
    /// needs the explicit wildcard mode, but no decoding or collation provider.
    /// Complete raw units use an explicitly selected UTF-16 provider; other
    /// providers retain strict Unicode publication before matching.
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

        let raw = matches!(value, ExpressionValue::StringBytes(_))
            || matches!(pattern, ExpressionValue::StringBytes(_));
        if raw
            && let Some(collation) = self.collation
            && collation.like_profile() == LikeProfile::Utf16Weights
        {
            self.check(0)?;
            let mode = self.like_mode.ok_or(EvalError::LikeModeRequired)?;
            let value = like_string(value, self.limits.max_text_bytes)?;
            let pattern = like_string(pattern, self.limits.max_text_bytes)?;
            let result = like_utf16(value, pattern, mode, collation, self.budget, self.cancelled)?;
            return Ok(if negated { result.negate() } else { result }.into_value());
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

fn like_string(
    value: &ExpressionValue,
    max_text_bytes: usize,
) -> Result<LikeString<'_>, EvalError> {
    match value {
        ExpressionValue::StringBytes(bytes) => Ok(LikeString::Utf16Le(bytes.as_bytes())),
        ExpressionValue::Value(Value::Text(text)) => {
            if text.len() > max_text_bytes {
                return Err(EvalError::TextLimit);
            }
            Ok(LikeString::Text(text))
        }
        ExpressionValue::Value(_) => Err(EvalError::TypeMismatch),
        _ => Err(EvalError::NonScalarExpression),
    }
}

#[cfg(test)]
mod tests;
