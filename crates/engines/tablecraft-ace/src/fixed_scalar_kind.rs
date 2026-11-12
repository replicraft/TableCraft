//! Exact native descriptors for retained fixed-scalar original witnesses.
use crate::format::data_type::{BYTE, DATE_TIME, DOUBLE, INT16, INT64, SINGLE};
use tablecraft_core::values::Value;

/// A data-only native scalar descriptor, not permission to modify a field.
///
/// Native admission independently verifies the complete original declaration,
/// source, record language, constraints and ownership before producing a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFixedScalarKind {
    /// Unsigned one-byte native Byte.
    Byte,
    /// Signed two-byte native Integer.
    Int16,
    /// Signed eight-byte native Large Number.
    Int64,
    /// Finite IEEE binary32 native Single.
    Single,
    /// Finite IEEE binary64 native Double.
    Double,
    /// Legacy eight-byte OLE DateTime, distinct from Extended DateTime.
    DateTime,
}

impl NativeFixedScalarKind {
    /// Exact native field tag; no Rust discriminant is serialized.
    pub const fn native_type(self) -> u8 {
        match self {
            Self::Byte => BYTE,
            Self::Int16 => INT16,
            Self::Int64 => INT64,
            Self::Single => SINGLE,
            Self::Double => DOUBLE,
            Self::DateTime => DATE_TIME,
        }
    }

    pub(crate) const fn from_native_type(native: u8) -> Option<Self> {
        match native {
            BYTE => Some(Self::Byte),
            INT16 => Some(Self::Int16),
            INT64 => Some(Self::Int64),
            SINGLE => Some(Self::Single),
            DOUBLE => Some(Self::Double),
            DATE_TIME => Some(Self::DateTime),
            _ => None,
        }
    }

    pub(crate) fn accepts(self, value: &Value) -> bool {
        match (self, value) {
            (_, Value::Null)
            | (Self::Byte, Value::Byte(_))
            | (Self::Int16, Value::Int16(_))
            | (Self::Int64, Value::Int64(_))
            | (Self::DateTime, Value::DateTime(_)) => true,
            (Self::Single, Value::Single(value)) => value.to_float().is_finite(),
            (Self::Double, Value::Double(value)) => value.to_float().is_finite(),
            _ => false,
        }
    }
}
