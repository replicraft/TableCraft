//! Language-only states and explicit conversion to stored scalars.

use std::fmt;

use super::{ExpressionState, Value, ValueError};

/// An expression carrier that cannot silently persist language-only states.
///
/// This is not a VBA interpreter. Error codes are opaque codes supplied by the
/// evaluator, without implementing CVErr conversion or exception handling.
#[derive(Clone, Eq, PartialEq)]
pub enum ExpressionValue {
    /// An ordinary scalar, including SQL NULL.
    Value(Value),
    /// Uninitialized language value.
    Empty,
    /// Omitted optional argument.
    Missing,
    /// Opaque evaluator error code.
    Error(u32),
}

impl TryFrom<ExpressionValue> for Value {
    type Error = ValueError;

    fn try_from(value: ExpressionValue) -> Result<Self, Self::Error> {
        match value {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Empty => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Empty))
            }
            ExpressionValue::Missing => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Missing))
            }
            ExpressionValue::Error(_) => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Error))
            }
        }
    }
}

impl fmt::Debug for ExpressionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.debug_tuple("Value").field(value).finish(),
            Self::Empty => f.write_str("Empty"),
            Self::Missing => f.write_str("Missing"),
            Self::Error(_) => f.write_str("Error([redacted])"),
        }
    }
}
