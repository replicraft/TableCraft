//! Inspect materialized carrier types and explicit expression-only states.
mod input;
mod kind;

use crate::EvalError;
pub use input::InspectionInput;
use tablecraft_core::values::{ExpressionValue, Value};

/// Closed type/state inspection calls, without COM/default-property resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueInspectionFunction {
    /// VBA-facing name of a supported materialized carrier.
    TypeName,
    /// VBA subtype code, including an array flag where applicable.
    VarType,
    /// Whether the explicit input is an array.
    IsArray,
    /// Whether the host supplied the distinct Empty state.
    IsEmpty,
    /// Whether the host supplied an opaque Error value, not an evaluation failure.
    IsError,
}

/// Allocation-free result; hosts charge and materialize borrowed type names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectionResult {
    /// A Boolean predicate or Integer subtype code.
    Scalar(Value),
    /// A static VBA-facing type name.
    TypeName(&'static str),
}

impl ValueInspectionFunction {
    /// ASCII-insensitive unqualified lookup without evaluating user code.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("TypeName", Self::TypeName),
            ("VarType", Self::VarType),
            ("IsArray", Self::IsArray),
            ("IsEmpty", Self::IsEmpty),
            ("IsError", Self::IsError),
        ]
        .into_iter()
        .find_map(|(n, function)| name.eq_ignore_ascii_case(n).then_some(function))
    }

    /// Inspect an already evaluated input without converting or scanning payloads.
    /// Missing is an optional-argument marker, not a first-class Error value here.
    pub fn apply(self, input: InspectionInput<'_>) -> Result<InspectionResult, EvalError> {
        let input = input.normalized();
        if matches!(input, InspectionInput::Expression(ExpressionValue::Missing)) {
            return Err(EvalError::UnsupportedExpression);
        }

        Ok(match self {
            Self::TypeName => InspectionResult::TypeName(kind::metadata(input)?.0),
            Self::VarType => InspectionResult::Scalar(Value::Int16(kind::metadata(input)?.1)),
            Self::IsArray => {
                InspectionResult::Scalar(Value::Boolean(matches!(input, InspectionInput::Array(_))))
            }
            Self::IsEmpty => InspectionResult::Scalar(Value::Boolean(matches!(
                input,
                InspectionInput::Expression(ExpressionValue::Empty)
            ))),
            Self::IsError => InspectionResult::Scalar(Value::Boolean(matches!(
                input,
                InspectionInput::Expression(ExpressionValue::Error(_))
            ))),
        })
    }
}

#[cfg(test)]
mod tests;
