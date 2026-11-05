//! Inert input-instruction tags; schema types still govern parsing and admission.
use tablecraft_schema::ScalarType;

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

impl From<ScalarType> for InputHint {
    fn from(kind: ScalarType) -> Self {
        match kind {
            ScalarType::Boolean => Self::Boolean,
            ScalarType::Byte => Self::Byte,
            ScalarType::Int16 => Self::Int16,
            ScalarType::Int32 => Self::Int32,
            ScalarType::Int64 => Self::Int64,
            ScalarType::Single => Self::Single,
            ScalarType::Double => Self::Double,
            ScalarType::Currency => Self::Currency,
            ScalarType::Decimal => Self::Decimal,
            ScalarType::Text => Self::Text,
            ScalarType::Binary => Self::BinaryHex,
            ScalarType::Guid => Self::Guid,
            ScalarType::DateTime => Self::DateTimeIso,
            ScalarType::DateTimeExtended => Self::DateTimeExtendedIso,
        }
    }
}
