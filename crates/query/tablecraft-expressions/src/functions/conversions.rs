//! Explicit conversion functions, separate from nullable internal coercion.
mod currency;
mod decimal;
pub mod errors;
mod floating;
pub mod guid;
pub mod text;
pub mod variant;
use crate::{
    EvalError, functions,
    numeric::{self, IntegerType},
};
use tablecraft_core::values::{Double, Value};

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
    /// CDec: checked 96-bit Decimal, separate from field precision limits.
    Decimal,
}
impl Conversion {
    /// Convert an entire numeric Text spelling under an explicit separator/currency
    /// profile. The initially empty buffer charges normalization before allocation.
    /// No prefix truncation or ambient locale lookup is performed.
    pub fn apply_text(
        self,
        value: &str,
        format: text::NumberFormat,
        buffer: &mut impl crate::functions::text::output::NumericBuffer,
    ) -> Result<Value, EvalError> {
        text::convert(self, value, format, buffer)
    }

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
            ("CDec", Self::Decimal),
        ]
        .into_iter()
        .find_map(|(spelling, conversion)| {
            name.eq_ignore_ascii_case(spelling).then_some(conversion)
        })
    }
    /// Convert a numeric/Boolean value or a legacy DATE's stored Double serial,
    /// checking range after half-even rounding for integer targets. NULL fails
    /// explicitly. Text requires apply_text; Extended dates, binary and GUID
    /// coercion remain unsupported. No result allocation is performed.
    pub fn apply(self, value: &Value) -> Result<Value, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
        let serial;
        let value = if let Value::DateTime(date) = value {
            // A legacy DATE's numeric representation is its OLE serial, not a
            // calendar-rounded value or a count of elapsed civil days.
            serial = Value::Double(Double::from_float(date.ole_days()));
            &serial
        } else {
            value
        };
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
            Self::Decimal => decimal::convert(value),
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod date_tests;
