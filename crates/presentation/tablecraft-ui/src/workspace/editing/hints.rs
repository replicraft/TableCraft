//! Localized instructions shared by cell edits and explicit new-row fields.
use crate::Localizer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputHint {
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    Single,
    Double,
    Currency,
    Decimal,
    Text,
    BinaryHex,
    Guid,
    DateTimeIso,
    DateTimeExtendedIso,
}

impl InputHint {
    pub(in crate::workspace) fn label(self, localizer: &Localizer) -> String {
        let key = match self {
            Self::Boolean => "editing-input-boolean",
            Self::Byte => "editing-input-byte",
            Self::Int16 => "editing-input-int16",
            Self::Int32 => "editing-input-int32",
            Self::Int64 => "editing-input-int64",
            Self::Single => "editing-input-single",
            Self::Double => "editing-input-double",
            Self::Currency => "editing-input-currency",
            Self::Decimal => "editing-input-decimal",
            Self::Text => "editing-input-text",
            Self::BinaryHex => "editing-input-binary",
            Self::Guid => "editing-input-guid",
            Self::DateTimeIso => "editing-input-date",
            Self::DateTimeExtendedIso => "editing-input-date-extended",
        };
        localizer.text(key)
    }
}
