//! Materialized scalars and redacted diagnostic output.

use std::fmt;

use super::{Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single};

/// Materialized scalar values. Field behavior and source preservation are external.
///
/// Integer widths and float bit patterns remain distinct. Text and binary are
/// owned materializations, not unbounded streaming APIs. Callers must enforce
/// resource budgets before allocating them. `Eq` compares carriers, not SQL
/// equality, collation, coercion, or three-valued NULL logic. Debug redacts data.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum Value {
    /// SQL NULL, distinct from zero-length text and VBA Empty.
    Null,
    /// A logical boolean, without VBA numeric coercion.
    Boolean(bool),
    /// Unsigned 8-bit Access Byte.
    Byte(u8),
    /// Signed 16-bit Integer.
    Int16(i16),
    /// Signed 32-bit Long Integer.
    Int32(i32),
    /// Signed 64-bit Large Number, subject to backend capability checks.
    Int64(i64),
    /// Single-precision float bits.
    Single(Single),
    /// Double-precision float bits.
    Double(Double),
    /// Exact fixed-scale money.
    Currency(Currency),
    /// Exact decimal with retained scale; declared field constraints apply separately.
    Decimal(Decimal),
    /// Decoded Unicode text; an empty string is not NULL.
    Text(String),
    /// Binary data without an assumed OLE, attachment or text interpretation.
    Binary(Vec<u8>),
    /// Canonical GUID value, not an internal object identity.
    Guid(Guid),
    /// Legacy timezone-free date/time.
    DateTime(LegacyDateTime),
    /// Extended timezone-free date/time.
    DateTimeExtended(ExtendedDateTime),
}

impl Value {
    /// Return a diagnostic-safe carrier name, not a native schema type code.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Boolean(_) => "Boolean",
            Self::Byte(_) => "Byte",
            Self::Int16(_) => "Int16",
            Self::Int32(_) => "Int32",
            Self::Int64(_) => "Int64",
            Self::Single(_) => "Single",
            Self::Double(_) => "Double",
            Self::Currency(_) => "Currency",
            Self::Decimal(_) => "Decimal",
            Self::Text(_) => "Text",
            Self::Binary(_) => "Binary",
            Self::Guid(_) => "Guid",
            Self::DateTime(_) => "DateTime",
            Self::DateTimeExtended(_) => "DateTimeExtended",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(self.kind()).field(&"[redacted]").finish()
    }
}
