use tablecraft_core::values::Value;
use thiserror::Error;

/// Payload-free rejection of explicitly supplied scalar field constraints.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FieldConstraintViolation {
    /// A required value is NULL, independently of empty-string rules.
    #[error("required field is NULL")]
    Required,
    /// A Text value is empty while zero-length strings are disallowed.
    #[error("field disallows zero-length text")]
    EmptyText,
}

/// Explicit scalar constraints, without expression/default evaluation or inferred
/// backend defaults. Storage adapters must resolve absent/unqualified metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldConstraints {
    required: bool,
    allow_zero_length: bool,
}

impl FieldConstraints {
    /// Supply both policies explicitly; all combinations have distinct semantics.
    pub const fn new(required: bool, allow_zero_length: bool) -> Self {
        Self {
            required,
            allow_zero_length,
        }
    }
    /// Validate a single exact carrier without coercing NULL/empty values.
    /// Non-Text carriers are unaffected by the zero-length-text policy.
    pub fn check(self, value: &Value) -> Result<(), FieldConstraintViolation> {
        if self.required && matches!(value, Value::Null) {
            return Err(FieldConstraintViolation::Required);
        }
        if !self.allow_zero_length && matches!(value,Value::Text(text) if text.is_empty()) {
            return Err(FieldConstraintViolation::EmptyText);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_empty_text_and_empty_binary_have_distinct_constraint_meanings() {
        let constraints = FieldConstraints::new(true, false);
        assert_eq!(
            constraints.check(&Value::Null),
            Err(FieldConstraintViolation::Required)
        );
        assert_eq!(
            constraints.check(&Value::Text(String::new())),
            Err(FieldConstraintViolation::EmptyText)
        );
        assert!(constraints.check(&Value::Binary(Vec::new())).is_ok());
        assert!(constraints.check(&Value::Int32(0)).is_ok());
        assert!(
            FieldConstraints::new(false, false)
                .check(&Value::Null)
                .is_ok()
        );
        assert!(
            FieldConstraints::new(true, true)
                .check(&Value::Text(String::new()))
                .is_ok()
        );
    }
}
