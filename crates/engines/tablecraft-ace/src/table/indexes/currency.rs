//! Exact Currency bounds; only the engine maps them to signed physical units.
use super::Int64IndexSelection;
use std::ops::Bound;
use tablecraft_core::values::Currency;

/// Selection for a qualified single-Currency index, independent of direction.
/// Bounds retain exact ten-thousandths and exclude NULL. Other key carriers and
/// composite indexes are rejected, including for All/Null selections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrencyIndexSelection {
    /// Complete traversal including NULL, with complete count validation.
    All,
    /// Only NULL keys; skipped pages and records are not audited.
    Null,
    /// Exact numeric interval; empty/reversed intervals match no rows.
    Range {
        /// Inclusive, exclusive or unbounded lower endpoint.
        lower: Bound<Currency>,
        /// Inclusive, exclusive or unbounded upper endpoint.
        upper: Bound<Currency>,
    },
}

impl CurrencyIndexSelection {
    pub(super) fn scaled(self) -> Int64IndexSelection {
        match self {
            Self::All => Int64IndexSelection::All,
            Self::Null => Int64IndexSelection::Null,
            Self::Range { lower, upper } => Int64IndexSelection::Range {
                lower: lower.map(Currency::scaled),
                upper: upper.map(Currency::scaled),
            },
        }
    }
}
