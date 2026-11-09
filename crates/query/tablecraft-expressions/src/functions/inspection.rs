//! Inspection of materialized scalar values without implicit runtime activation.
mod date;
mod numeric;
pub mod types;
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat, dates::parsing::DateParseContext,
        text::output::NumericBuffer,
    },
};
use tablecraft_core::values::Value;

/// Closed inspection calls. Runtime objects, arrays and language-only states are absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionFunction {
    /// Numeric/Boolean type recognition and explicit-policy whole-Text recognition.
    IsNumeric,
    /// Recognize supported dates/times with explicit Gregorian Text policies.
    IsDate,
}
impl InspectionFunction {
    /// Resolve one unqualified ASCII-insensitive function identity.
    pub fn resolve(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("IsNumeric") {
            Some(Self::IsNumeric)
        } else {
            name.eq_ignore_ascii_case("IsDate").then_some(Self::IsDate)
        }
    }
    /// Inspect one already evaluated value under an explicit numeric Text policy.
    /// The initially empty buffer owns charged normalization; discard on failure.
    pub fn apply(
        self,
        value: &Value,
        format: Option<NumberFormat>,
        buffer: &mut impl NumericBuffer,
    ) -> Result<Value, EvalError> {
        self.apply_with_context(
            value,
            DateParseContext {
                number_format: format,
                ..Default::default()
            },
            buffer,
        )
    }
    /// Inspect using request-local numeric and date recognition policies.
    /// Missing policy and host failures remain errors rather than FALSE.
    pub fn apply_with_context(
        self,
        value: &Value,
        context: DateParseContext,
        buffer: &mut impl NumericBuffer,
    ) -> Result<Value, EvalError> {
        buffer.step()?;
        if !buffer.text().is_empty() {
            return Err(EvalError::Domain);
        }
        match self {
            Self::IsNumeric => {
                numeric::evaluate(value, context.number_format, buffer).map(Value::Boolean)
            }
            Self::IsDate => date::evaluate(value, context, buffer).map(Value::Boolean),
        }
    }
}
