//! Lazy publication at the owning date operation's admission position.
use super::Evaluator;
use crate::EvalError;
use std::borrow::Cow;
use tablecraft_core::values::{ExpressionValue, Value};

#[derive(Clone, Copy)]
pub(super) enum DateInput<'a> {
    Scalar(&'a Value),
    Expression(&'a ExpressionValue),
}

impl<'a> DateInput<'a> {
    pub(super) fn scalar(self) -> Option<&'a Value> {
        match self {
            Self::Scalar(value) | Self::Expression(ExpressionValue::Value(value)) => Some(value),
            _ => None,
        }
    }

    pub(super) fn materialize(
        self,
        evaluator: &mut Evaluator<'_>,
    ) -> Result<Cow<'a, Value>, EvalError> {
        match self {
            Self::Scalar(value) | Self::Expression(ExpressionValue::Value(value)) => {
                Ok(Cow::Borrowed(value))
            }
            Self::Expression(ExpressionValue::StringBytes(value)) => {
                evaluator.publish_expression_string(value).map(Cow::Owned)
            }
            Self::Expression(ExpressionValue::Empty) => Ok(Cow::Owned(Value::Int16(0))),
            Self::Expression(ExpressionValue::Error(_)) => Err(EvalError::TypeMismatch),
            Self::Expression(ExpressionValue::Missing) => Err(EvalError::NonScalarExpression),
        }
    }
}
