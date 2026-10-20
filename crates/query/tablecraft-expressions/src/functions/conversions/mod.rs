//! Explicit conversion functions, separate from nullable internal coercion.
mod currency;
mod floating;
use crate::{
    EvalError, functions,
    numeric::{self, IntegerType},
};
use tablecraft_core::values::Value;

/// Closed numeric/Boolean conversion identities; locale parsing is not inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    /// CByte: unsigned 8-bit result, with the Boolean TRUE -> 255 exception.
    Byte,
    /// CInt: signed 16-bit Integer result.
    Integer,
    /// CLng: signed 32-bit Long result.
    Long,
    /// CLngLng: signed 64-bit result, independent of host pointer width.
    LongLong,
    /// CBool: numeric zero/nonzero truth, preserving exact numeric precision.
    Boolean,
    /// CSng: explicit IEEE binary32 rounding with finite/range checks.
    Single,
    /// CDbl: explicit IEEE binary64 rounding with finite checks.
    Double,
    /// CCur: checked four-decimal Currency with half-even rounding.
    Currency,
}
impl Conversion {
    /// ASCII-insensitive unqualified lookup, without allocation or host execution.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("CByte", Self::Byte),
            ("CInt", Self::Integer),
            ("CLng", Self::Long),
            ("CLngLng", Self::LongLong),
            ("CBool", Self::Boolean),
            ("CSng", Self::Single),
            ("CDbl", Self::Double),
            ("CCur", Self::Currency),
        ]
        .into_iter()
        .find_map(|(spelling, conversion)| {
            name.eq_ignore_ascii_case(spelling).then_some(conversion)
        })
    }
    /// Convert one evaluated numeric/Boolean value, checking range after half-even
    /// rounding for integer targets. NULL fails explicitly; internal coercion's
    /// nullable behavior is not exposed as function semantics. Date/text/binary/
    /// GUID coercion remains unsupported. No result allocation is performed.
    pub fn apply(self, value: &Value) -> Result<Value, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
        match self {
            Self::Byte if matches!(value, Value::Boolean(_)) => {
                Ok(Value::Byte(if matches!(value, Value::Boolean(true)) {
                    u8::MAX
                } else {
                    u8::MIN
                }))
            }
            Self::Byte => numeric::coerce_integer(value, IntegerType::Byte),
            Self::Integer => numeric::coerce_integer(value, IntegerType::Int16),
            Self::Long => numeric::coerce_integer(value, IntegerType::Int32),
            Self::LongLong => numeric::coerce_integer(value, IntegerType::Int64),
            Self::Boolean => functions::iif(value, Value::Boolean(true), Value::Boolean(false)),
            Self::Single => floating::single(value),
            Self::Double => floating::double(value),
            Self::Currency => currency::convert(value),
        }
    }
}
#[cfg(test)]
mod tests;
