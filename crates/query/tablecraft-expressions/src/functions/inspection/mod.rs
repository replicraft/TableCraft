//! Inspection of materialized scalar values without implicit runtime activation.
mod numeric;
use crate::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::output::NumericBuffer},
};
use tablecraft_core::values::Value;

/// Closed inspection calls. Runtime objects, arrays and language-only states are absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionFunction {
    /// Numeric/Boolean type recognition and explicit-policy whole-Text recognition.
    IsNumeric,
}
impl InspectionFunction {
    /// Resolve one unqualified ASCII-insensitive function identity.
    pub fn resolve(name: &str) -> Option<Self> {
        name.eq_ignore_ascii_case("IsNumeric")
            .then_some(Self::IsNumeric)
    }
    /// Inspect one already evaluated value under an explicit numeric Text policy.
    /// The initially empty buffer owns charged normalization; discard on failure.
    pub fn apply(
        self,
        value: &Value,
        format: Option<NumberFormat>,
        buffer: &mut impl NumericBuffer,
    ) -> Result<Value, EvalError> {
        buffer.step()?;
        if !buffer.text().is_empty() {
            return Err(EvalError::Domain);
        }
        match self {
            Self::IsNumeric => numeric::evaluate(value, format, buffer).map(Value::Boolean),
        }
    }
}
