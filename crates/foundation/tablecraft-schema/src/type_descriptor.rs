use crate::{ScalarType, SchemaError};
use std::num::NonZeroUsize;
use tablecraft_core::values::Value;

/// Reusable declared scalar type and currently supported value constraints.
/// Nullability belongs to the field/parameter owner; resource ceilings are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeDescriptor {
    scalar: ScalarType,
    text_length: Option<NonZeroUsize>,
}
impl TypeDescriptor {
    /// Exact carrier without additional declared constraints.
    pub const fn new(scalar: ScalarType) -> Self {
        Self {
            scalar,
            text_length: None,
        }
    }
    /// Text with a positive Unicode-scalar length ceiling, preserving the current
    /// query profile's units. Native character-unit fidelity is unqualified.
    pub fn text(max_characters: usize) -> Result<Self, SchemaError> {
        Ok(Self {
            scalar: ScalarType::Text,
            text_length: Some(
                NonZeroUsize::new(max_characters).ok_or(SchemaError::TypeConstraint)?,
            ),
        })
    }
    /// Required non-NULL carrier; no conversion is performed.
    pub const fn scalar_type(self) -> ScalarType {
        self.scalar
    }
    /// Declared Text length in Unicode scalar values, if constrained.
    pub const fn max_text_characters(self) -> Option<usize> {
        match self.text_length {
            Some(n) => Some(n.get()),
            None => None,
        }
    }
    /// Validate carrier/declared constraints. NULL passes here; the owner checks
    /// nullability. Non-finite carriers and allocation limits are not schema rules.
    pub fn validate_value(self, value: &Value) -> Result<(), SchemaError> {
        if ScalarType::of(value).is_some_and(|kind| kind != self.scalar)
            || matches!(value, Value::Text(t) if self.text_length.is_some_and(|n|t.chars().nth(n.get()).is_some()))
        {
            return Err(SchemaError::Value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_types_validate_without_coercion_or_resource_policy() {
        assert_eq!(TypeDescriptor::text(0), Err(SchemaError::TypeConstraint));
        let text = TypeDescriptor::text(2).unwrap();
        assert!(text.validate_value(&Value::Text("é🦀".into())).is_ok());
        assert_eq!(
            text.validate_value(&Value::Text("abc".into())),
            Err(SchemaError::Value)
        );
        assert!(text.validate_value(&Value::Null).is_ok());
        assert_eq!(
            text.validate_value(&Value::Int64(2)),
            Err(SchemaError::Value)
        );
        assert!(
            TypeDescriptor::new(ScalarType::Text)
                .validate_value(&Value::Text("x".repeat(65_537)))
                .is_ok()
        );
    }
}
