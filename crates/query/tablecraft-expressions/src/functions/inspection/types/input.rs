use crate::{arrays::ArrayView, functions::text::byte_string::ByteString};
use tablecraft_core::values::{ExpressionValue, Value};

/// Borrowed inspection input; no coercion, storage conversion or object activation.
#[derive(Clone, Copy)]
pub enum InspectionInput<'a> {
    /// An ordinary materialized scalar, including NULL or binary bytes.
    Scalar(&'a Value),
    /// Explicit language-only state or scalar wrapper supplied by a host.
    Expression(&'a ExpressionValue),
    /// An explicit array, separate from a stored field carrier.
    Array(ArrayView<'a>),
    /// Lossless expression string bytes; still String, never an array.
    ByteString(&'a ByteString),
}

impl InspectionInput<'_> {
    pub(super) fn normalized(self) -> Self {
        match self {
            Self::Expression(ExpressionValue::Value(value)) => Self::Scalar(value).normalized(),
            Self::Scalar(Value::Binary(bytes)) => Self::Array(ArrayView::Bytes(bytes)),
            Self::Scalar(Value::Guid(guid)) => Self::Array(ArrayView::Guid(guid)),
            _ => self,
        }
    }
}
