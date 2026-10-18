use std::ops::Bound;

/// Optional backend-neutral restriction on one exact Int32 source column.
/// Ordinals refer to Source::columns; numeric bounds exclude NULL and use
/// ascending value order. Empty/reversed intervals match no rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanConstraint {
    /// Select NULL values in the specified column.
    Null {
        /// Zero-based source column ordinal.
        column: usize,
    },
    /// Select a numeric interval in the specified column.
    Int32 {
        /// Zero-based source column ordinal.
        column: usize,
        /// Numeric lower bound.
        lower: Bound<i32>,
        /// Numeric upper bound.
        upper: Bound<i32>,
    },
}
impl ScanConstraint {
    /// Zero-based source column ordinal.
    pub const fn column(self) -> usize {
        match self {
            Self::Null { column } | Self::Int32 { column, .. } => column,
        }
    }
}
