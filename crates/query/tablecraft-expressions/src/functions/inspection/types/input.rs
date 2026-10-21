use crate::arrays::ArrayView;
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
}

impl InspectionInput<'_> {
    pub(super) fn normalized(self) -> Self {
        match self {
            Self::Expression(ExpressionValue::Value(value)) => Self::Scalar(value).normalized(),
            Self::Scalar(Value::Binary(bytes)) => Self::Array(ArrayView::Bytes(bytes)),
            _ => self,
        }
    }
}
