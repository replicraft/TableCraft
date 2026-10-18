use tablecraft_core::values::Value;

/// Exact materialized carrier; not a native field layout or coercion policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarType {
    /// Boolean.
    Boolean,
    /// Unsigned byte.
    Byte,
    /// Signed 16-bit integer.
    Int16,
    /// Signed 32-bit integer.
    Int32,
    /// Signed 64-bit integer.
    Int64,
    /// Single precision.
    Single,
    /// Double precision.
    Double,
    /// Fixed-scale currency.
    Currency,
    /// Exact decimal.
    Decimal,
    /// Unicode text.
    Text,
    /// Binary bytes.
    Binary,
    /// GUID.
    Guid,
    /// Legacy date/time.
    DateTime,
    /// Extended date/time.
    DateTimeExtended,
}
impl ScalarType {
    /// Carrier of a non-NULL value; NULL has no inferred scalar type.
    pub const fn of(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Null => return None,
            Value::Boolean(_) => Self::Boolean,
            Value::Byte(_) => Self::Byte,
            Value::Int16(_) => Self::Int16,
            Value::Int32(_) => Self::Int32,
            Value::Int64(_) => Self::Int64,
            Value::Single(_) => Self::Single,
            Value::Double(_) => Self::Double,
            Value::Currency(_) => Self::Currency,
            Value::Decimal(_) => Self::Decimal,
            Value::Text(_) => Self::Text,
            Value::Binary(_) => Self::Binary,
            Value::Guid(_) => Self::Guid,
            Value::DateTime(_) => Self::DateTime,
            Value::DateTimeExtended(_) => Self::DateTimeExtended,
        })
    }
}
