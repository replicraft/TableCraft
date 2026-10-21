use super::ArrayValue;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

/// Borrowed array input, keeping opaque binary data distinct from Variant elements.
#[derive(Clone, Copy)]
pub enum ArrayView<'a> {
    /// Explicit one-dimensional Variant-element array.
    Variants(&'a ArrayValue),
    /// Materialized binary interpreted as a zero-based Byte array for inspection.
    Bytes(&'a [u8]),
}

/// Closed dimension-bound calls over explicit array inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundFunction {
    /// Smallest available subscript.
    LBound,
    /// Largest available subscript.
    UBound,
}

impl BoundFunction {
    /// ASCII-insensitive lookup without provider or runtime activation.
    pub fn resolve(name: &str) -> Option<Self> {
        [("LBound", Self::LBound), ("UBound", Self::UBound)]
            .into_iter()
            .find_map(|(n, function)| name.eq_ignore_ascii_case(n).then_some(function))
    }

    /// Inspect the single dimension, defaulting to one only when omitted.
    /// Numeric dimension conversion uses the existing checked CLng rules.
    pub fn apply(
        self,
        array: ArrayView<'_>,
        dimension: Option<&Value>,
    ) -> Result<Value, EvalError> {
        let dimension = match dimension {
            None => 1,
            Some(value) => {
                let Value::Int32(value) = Conversion::Long.apply(value)? else {
                    return Err(EvalError::TypeMismatch);
                };
                value
            }
        };
        if dimension != 1 {
            return Err(EvalError::ArrayBounds);
        }

        let (lower, upper) = match array {
            ArrayView::Variants(array) => (array.lower_bound(), array.upper_bound()),
            ArrayView::Bytes(bytes) => {
                let upper = if bytes.is_empty() {
                    -1
                } else {
                    i32::try_from(bytes.len() - 1).map_err(|_| EvalError::ArrayBounds)?
                };
                (0, upper)
            }
        };

        Ok(Value::Int32(match self {
            Self::LBound => lower,
            Self::UBound => upper,
        }))
    }
}
