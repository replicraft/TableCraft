//! Validated single-field type facts, without native offsets or write authority.
use super::FieldAdditionKind;
use std::num::NonZeroU8;
use tablecraft_schema::{ScalarType, SchemaError, TypeDescriptor};
use thiserror::Error;

/// Invalid saved-field declaration; no input text or native source is retained.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FieldAdditionDeclarationError {
    #[error("this scalar type is not a saved-field gallery declaration")]
    UnsupportedType,
    #[error(transparent)]
    Constraint(#[from] SchemaError),
}

/// One ordinary field under the gallery's explicit type-default policy.
/// Required(false), absent Date/GUID/Text defaults, exact zero numeric defaults
/// and Short Text AllowZeroLength(true) are fixed policy, not executable input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedFieldAdditionDeclaration {
    kind: FieldAdditionKind,
    descriptor: TypeDescriptor,
    text_units: Option<NonZeroU8>,
    compression: bool,
}

/// Positive source advice for one default configuration; this is not write authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedFieldAdditionAvailability {
    pub declaration: TypedFieldAdditionDeclaration,
    pub maximum_name_utf16_units: usize,
}

impl TypedFieldAdditionDeclaration {
    /// Validate one of the ten fixed scalar gallery types.
    pub fn fixed(scalar: ScalarType) -> Result<Self, FieldAdditionDeclarationError> {
        let kind = match scalar {
            ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Guid => FieldAdditionKind::Number,
            ScalarType::Int64 => FieldAdditionKind::LargeInteger,
            ScalarType::Currency => FieldAdditionKind::Currency,
            ScalarType::Boolean => FieldAdditionKind::YesNo,
            ScalarType::DateTime => FieldAdditionKind::DateTime,
            _ => return Err(FieldAdditionDeclarationError::UnsupportedType),
        };
        Ok(Self::gallery_fixed(kind, scalar))
    }

    pub(super) const fn gallery_fixed(kind: FieldAdditionKind, scalar: ScalarType) -> Self {
        Self {
            kind,
            descriptor: TypeDescriptor::new(scalar),
            text_units: None,
            compression: false,
        }
    }

    /// Preserve the declared Decimal precision and scale without normalization.
    pub fn decimal(precision: u8, scale: u8) -> Result<Self, FieldAdditionDeclarationError> {
        Ok(Self {
            kind: FieldAdditionKind::Number,
            descriptor: TypeDescriptor::decimal(precision, scale)?,
            text_units: None,
            compression: false,
        })
    }

    /// Native UTF-16 field units remain separate from query scalar-count limits.
    pub const fn short_text(units: NonZeroU8, unicode_compression: bool) -> Self {
        Self {
            kind: FieldAdditionKind::ShortText,
            descriptor: TypeDescriptor::new(ScalarType::Text),
            text_units: Some(units),
            compression: unicode_compression,
        }
    }

    pub const fn kind(self) -> FieldAdditionKind {
        self.kind
    }
    pub const fn descriptor(self) -> TypeDescriptor {
        self.descriptor
    }
    pub const fn short_text_units(self) -> Option<NonZeroU8> {
        self.text_units
    }
    pub const fn unicode_compression(self) -> bool {
        self.compression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_types_and_native_text_units_are_validated_without_query_length_aliases() {
        for scalar in [
            ScalarType::Boolean,
            ScalarType::Byte,
            ScalarType::Int16,
            ScalarType::Int32,
            ScalarType::Int64,
            ScalarType::Currency,
            ScalarType::Single,
            ScalarType::Double,
            ScalarType::DateTime,
            ScalarType::Guid,
        ] {
            let declaration = TypedFieldAdditionDeclaration::fixed(scalar).unwrap();
            assert_eq!(declaration.descriptor().scalar_type(), scalar);
            assert_eq!(declaration.short_text_units(), None);
        }
        for scalar in [
            ScalarType::Decimal,
            ScalarType::Text,
            ScalarType::Binary,
            ScalarType::DateTimeExtended,
        ] {
            assert_eq!(
                TypedFieldAdditionDeclaration::fixed(scalar),
                Err(FieldAdditionDeclarationError::UnsupportedType)
            );
        }
        let decimal = TypedFieldAdditionDeclaration::decimal(28, 4).unwrap();
        let constraints = decimal.descriptor().decimal_constraints().unwrap();
        assert_eq!((constraints.precision(), constraints.scale()), (28, 4));
        for (precision, scale) in [(0, 0), (29, 0), (3, 4)] {
            assert!(TypedFieldAdditionDeclaration::decimal(precision, scale).is_err());
        }
        let text = TypedFieldAdditionDeclaration::short_text(NonZeroU8::new(80).unwrap(), false);
        assert_eq!(text.short_text_units().unwrap().get(), 80);
        assert_eq!(text.descriptor().max_text_characters(), None);
        assert!(!text.unicode_compression());
    }
}
