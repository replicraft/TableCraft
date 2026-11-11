//! Explicit source-free declarations for the saved-field gallery's working choices.
mod typed;
pub use typed::{
    FieldAdditionDeclarationError, TypedFieldAdditionAvailability, TypedFieldAdditionDeclaration,
};

use std::num::NonZeroU8;
use tablecraft_schema::ScalarType;

/// Gallery choice, not a native source capability or arbitrary field declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionKind {
    /// The current saved-field Number gallery policy.
    Number,
    /// The current saved-field Short Text gallery policy.
    ShortText,
    Currency,
    YesNo,
    LargeInteger,
    DateTime,
}

/// Complete request semantics retained through inspection, review and save.
/// Number means LONG, stored Required(false) and typed Int32 zero. Short Text
/// means stored Required(false), AllowZeroLength(true) and absent DefaultValue,
/// together with the exact width/compression below. Native proof stays separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionDeclaration {
    /// Validated twelve-kind single-field declaration under type-default policy.
    Typed(TypedFieldAdditionDeclaration),
    /// Nullable LONG, stored Required(false), typed default zero.
    NumberLongZero,
    /// Nullable Text, stored Required(false)/AllowZeroLength(true), no default.
    ShortText {
        max_utf16_units: NonZeroU8,
        unicode_compression: bool,
    },
}

impl FieldAdditionDeclaration {
    pub const fn kind(self) -> FieldAdditionKind {
        match self {
            Self::Typed(declaration) => declaration.kind(),
            Self::NumberLongZero => FieldAdditionKind::Number,
            Self::ShortText { .. } => FieldAdditionKind::ShortText,
        }
    }

    /// Explicit current gallery policy; this never changes initial-table defaults.
    pub const fn ui_default(kind: FieldAdditionKind) -> Self {
        match kind {
            FieldAdditionKind::Number => Self::NumberLongZero,
            FieldAdditionKind::Currency => {
                Self::Typed(TypedFieldAdditionDeclaration::gallery_fixed(
                    FieldAdditionKind::Currency,
                    ScalarType::Currency,
                ))
            }
            FieldAdditionKind::YesNo => Self::Typed(TypedFieldAdditionDeclaration::gallery_fixed(
                FieldAdditionKind::YesNo,
                ScalarType::Boolean,
            )),
            FieldAdditionKind::LargeInteger => {
                Self::Typed(TypedFieldAdditionDeclaration::gallery_fixed(
                    FieldAdditionKind::LargeInteger,
                    ScalarType::Int64,
                ))
            }
            FieldAdditionKind::DateTime => {
                Self::Typed(TypedFieldAdditionDeclaration::gallery_fixed(
                    FieldAdditionKind::DateTime,
                    ScalarType::DateTime,
                ))
            }
            FieldAdditionKind::ShortText => Self::ShortText {
                max_utf16_units: NonZeroU8::MAX,
                unicode_compression: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldAdditionDeclaration as Declaration, FieldAdditionKind as Kind};
    use std::num::NonZeroU8;

    #[test]
    fn gallery_policies_are_explicit_and_width_and_compression_remain_identity() {
        assert_eq!(
            Declaration::ui_default(Kind::Number),
            Declaration::NumberLongZero
        );
        let text = Declaration::ui_default(Kind::ShortText);
        assert_eq!(
            text,
            Declaration::ShortText {
                max_utf16_units: NonZeroU8::new(255).unwrap(),
                unicode_compression: true,
            }
        );
        assert_eq!(text.kind(), Kind::ShortText);
        assert_ne!(
            text,
            Declaration::ShortText {
                max_utf16_units: NonZeroU8::new(80).unwrap(),
                unicode_compression: true,
            }
        );
        assert_ne!(
            text,
            Declaration::ShortText {
                max_utf16_units: NonZeroU8::new(255).unwrap(),
                unicode_compression: false,
            }
        );
    }
}
