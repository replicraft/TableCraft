//! LIKE preparation separates String coercion from comparison-provider activation.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::LikeProfile,
    predicates::{LikeString, like_utf16},
};
use tablecraft_core::values::{ExpressionString, ExpressionValue, Value};

mod preparation;

/// Checked LIKE operands. Existing Strings are borrowed; only freshly formatted
/// non-String operands are owned. Prepare once before selecting a host provider.
pub struct PreparedLikeOperands<'a> {
    operands: Option<(Operand<'a>, Operand<'a>)>,
}

impl PreparedLikeOperands<'_> {
    /// NULL propagation requires neither String conversion nor a provider.
    pub fn is_null(&self) -> bool {
        self.operands.is_none()
    }

    /// Whether the source value acquired fresh String provenance by conversion.
    pub fn value_converted(&self) -> bool {
        matches!(self.operands, Some((Operand::Converted(_), _)))
    }

    /// Whether the pattern acquired fresh String provenance by conversion.
    pub fn pattern_converted(&self) -> bool {
        matches!(self.operands, Some((_, Operand::Converted(_))))
    }
}

enum Operand<'a> {
    Scalar(&'a Value),
    Raw(&'a ExpressionString),
    Converted(Value),
}

impl Operand<'_> {
    fn scalar(&self) -> Option<&Value> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Converted(value) => Some(value),
            Self::Raw(_) => None,
        }
    }

    fn string(&self) -> Result<LikeString<'_>, EvalError> {
        match self {
            Self::Raw(value) => Ok(LikeString::Utf16Le(value.as_bytes())),
            Self::Scalar(Value::Text(value)) | Self::Converted(Value::Text(value)) => {
                Ok(LikeString::Text(value))
            }
            _ => Err(EvalError::TypeMismatch),
        }
    }
}

impl Evaluator<'_> {
    /// Coerce admitted operands once, then match under the selected provider.
    /// Raw Strings preserve their active bytes and existing strict/UTF-16 paths.
    pub fn apply_expression_like(
        &mut self,
        value: &ExpressionValue,
        pattern: &ExpressionValue,
        negated: bool,
    ) -> Result<Value, EvalError> {
        let prepared = self.prepare_expression_like(value, pattern)?;
        self.apply_prepared_like(&prepared, negated)
    }

    /// Match prepared operands without repeating coercion or its resource charges.
    pub fn apply_prepared_like(
        &mut self,
        prepared: &PreparedLikeOperands<'_>,
        negated: bool,
    ) -> Result<Value, EvalError> {
        self.like_mode.ok_or(EvalError::LikeModeRequired)?;
        let Some((value, pattern)) = &prepared.operands else {
            return Ok(Value::Null);
        };

        let raw = matches!(value, Operand::Raw(_)) || matches!(pattern, Operand::Raw(_));
        if raw
            && let Some(collation) = self.collation
            && collation.like_profile() == LikeProfile::Utf16Weights
        {
            self.check(0)?;
            let mode = self.like_mode.ok_or(EvalError::LikeModeRequired)?;
            let result = like_utf16(
                value.string()?,
                pattern.string()?,
                mode,
                collation,
                self.budget,
                self.cancelled,
            )?;
            return Ok(if negated { result.negate() } else { result }.into_value());
        }

        let published_value = self.publish_like_raw(value)?;
        let published_pattern = self.publish_like_raw(pattern)?;
        let value = published_value.as_ref().or_else(|| value.scalar());
        let pattern = published_pattern.as_ref().or_else(|| pattern.scalar());
        let (Some(value), Some(pattern)) = (value, pattern) else {
            return Err(EvalError::TypeMismatch);
        };
        self.match_like_strings(value, pattern, negated)
    }

    fn publish_like_raw(&mut self, operand: &Operand<'_>) -> Result<Option<Value>, EvalError> {
        match operand {
            Operand::Raw(value) => self.publish_expression_string(value).map(Some),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coercion_tests;
