use std::ops::Bound;
use tablecraft_core::values::Guid;

/// Optional backend-neutral restriction on one exact integer or GUID source column.
/// Ordinals refer to Source::columns; numeric bounds exclude NULL and use
/// ascending value order. Empty/reversed intervals match no rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanConstraint {
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
    /// Select an Int32 interval in the specified Int32 column.
    Int32 {
        /// Zero-based source column ordinal.
        column: usize,
        /// Numeric lower bound.
        lower: Bound<i32>,
        /// Numeric upper bound.
        upper: Bound<i32>,
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
            | Self::Guid { column, .. }
            | Self::Int32 { column, .. }
            | Self::Int64 { column, .. } => column,
        }
    }
}
