use super::TextEqualityKey;
use std::ops::Bound;
use tablecraft_core::values::{Currency, Guid};

/// Optional backend-neutral restriction on one source column.
/// Typed intervals/equality apply to exact integer, Currency or GUID carriers;
/// NULL tests may also target other carriers supported by the source. Ordinals
/// refer to Source::columns. Numeric bounds exclude NULL and use ascending value
/// order. Empty/reversed intervals match no rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanConstraint {
    /// Conservative Text equality candidates under a resolver/source agreement.
    /// The source may return extra rows; the original equality remains required.
    TextEqual {
        /// Zero-based source column ordinal.
        column: usize,
        /// Opaque key issued by the actual selected comparer.
        key: TextEqualityKey,
    },
    /// Select NULL values in the specified column.
    Null {
        /// Zero-based source column ordinal.
        column: usize,
    },
    /// Select a canonical GUID by exact equality, excluding NULL.
    Guid {
        /// Zero-based source column ordinal.
        column: usize,
        /// Exact canonical GUID value.
        value: Guid,
    },
    /// Select an exact Byte interval in a Byte column.
    Byte {
        /// Zero-based source column ordinal.
        column: usize,
        /// Inclusive, exclusive or unbounded lower endpoint.
        lower: Bound<u8>,
        /// Inclusive, exclusive or unbounded upper endpoint.
        upper: Bound<u8>,
    },
    /// Select an exact Int16 interval in an Int16 column.
    Int16 {
        /// Zero-based source column ordinal.
        column: usize,
        /// Inclusive, exclusive or unbounded lower endpoint.
        lower: Bound<i16>,
        /// Inclusive, exclusive or unbounded upper endpoint.
        upper: Bound<i16>,
    },
    /// Select an Int32 interval in the specified Int32 column.
    Int32 {
        /// Zero-based source column ordinal.
        column: usize,
        /// Numeric lower bound.
        lower: Bound<i32>,
        /// Numeric upper bound.
        upper: Bound<i32>,
    },
    /// Select an exact Currency interval without floating-point conversion.
    Currency {
        /// Zero-based source column ordinal.
        column: usize,
        /// Exact lower bound in Currency units.
        lower: Bound<Currency>,
        /// Exact upper bound in Currency units.
        upper: Bound<Currency>,
    },
    /// Select an exact Int64 interval in the specified Int64 column.
    Int64 {
        /// Zero-based source column ordinal.
        column: usize,
        /// Numeric lower bound, with no floating conversion.
        lower: Bound<i64>,
        /// Numeric upper bound, with no floating conversion.
        upper: Bound<i64>,
    },
}
impl ScanConstraint {
    /// Zero-based source column ordinal.
    pub const fn column(self) -> usize {
        match self {
            Self::Null { column }
            | Self::TextEqual { column, .. }
            | Self::Guid { column, .. }
            | Self::Byte { column, .. }
            | Self::Int16 { column, .. }
            | Self::Int32 { column, .. }
            | Self::Int64 { column, .. }
            | Self::Currency { column, .. } => column,
        }
    }
}
