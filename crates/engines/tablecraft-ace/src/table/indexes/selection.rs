//! Exact logical integer bounds, independent of native key encoding/direction.
use super::page::KeyRef;
use std::ops::Bound;

/// Selection for a width-qualified native integer index reader.
/// Range bounds use numeric ascending order regardless of index direction.
/// This is a storage selection, without SQL coercion or BETWEEN normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerIndexSelection<T> {
    /// All keys, including NULL; retains complete traversal/count validation.
    All,
    /// Only NULL keys.
    Null,
    /// A numeric interval excluding NULL. Reversed/empty bounds return no rows.
    Range {
        /// Inclusive, exclusive or unbounded numeric lower endpoint.
        lower: Bound<T>,
        /// Inclusive, exclusive or unbounded numeric upper endpoint.
        upper: Bound<T>,
    },
}

/// Exact Int32 bounds; accepted only by the single-Int32 range reader.
pub type Int32IndexSelection = IntegerIndexSelection<i32>;
/// Exact Int64 bounds; accepted only by the qualified single-Int64 range reader.
pub type Int64IndexSelection = IntegerIndexSelection<i64>;

impl<T: Copy + Ord + Into<i64>> IntegerIndexSelection<T> {
    pub(super) fn widened(self) -> Int64IndexSelection {
        match self {
            Self::All => Int64IndexSelection::All,
            Self::Null => Int64IndexSelection::Null,
            Self::Range { lower, upper } => Int64IndexSelection::Range {
                lower: lower.map(Into::into),
                upper: upper.map(Into::into),
            },
        }
    }
    pub(super) fn contains(self, key: Option<i64>) -> bool {
        match (self, key) {
            (Self::All, _) | (Self::Null, None) => true,
            (Self::Range { lower, upper }, Some(value)) => {
                !below(value, lower) && !above(value, upper)
            }
            _ => false,
        }
    }

    fn empty(self) -> bool {
        use Bound::{Excluded, Included, Unbounded};
        match self {
            Self::Range { lower, upper } => match (lower, upper) {
                (Unbounded, _) | (_, Unbounded) => false,
                (Included(a), Included(b)) => a > b,
                (Included(a) | Excluded(a), Included(b) | Excluded(b)) => a >= b,
            },
            _ => false,
        }
    }

    /// Coarse inclusive key bounds deliberately retain duplicate boundary keys:
    /// parent separators also carry row references and can outlive deleted keys.
    pub(super) fn may_match(
        self,
        lower: Option<KeyRef>,
        upper: Option<KeyRef>,
        descending: bool,
    ) -> bool {
        if self.empty() {
            return false;
        }
        let (minimum, maximum) = if descending {
            (upper, lower)
        } else {
            (lower, upper)
        };
        match self {
            Self::All => true,
            Self::Null => minimum.is_none_or(|entry| entry.key.is_none()),
            Self::Range { lower, upper } => {
                if let Some(maximum) = maximum {
                    match maximum.key {
                        None => return false,
                        Some(value) if below(value, lower) => return false,
                        _ => {}
                    }
                }
                !minimum
                    .and_then(|entry| entry.key)
                    .is_some_and(|value| above(value, upper))
            }
        }
    }
}

fn below<T: Into<i64>>(value: i64, lower: Bound<T>) -> bool {
    match lower {
        Bound::Included(bound) => value < bound.into(),
        Bound::Excluded(bound) => value <= bound.into(),
        Bound::Unbounded => false,
    }
}

fn above<T: Into<i64>>(value: i64, upper: Bound<T>) -> bool {
    match upper {
        Bound::Included(bound) => value > bound.into(),
        Bound::Excluded(bound) => value >= bound.into(),
        Bound::Unbounded => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Bound::{Excluded, Included, Unbounded};

    #[test]
    fn null_extremes_empty_and_duplicate_boundary_keys_are_explicit() {
        let exact = Int32IndexSelection::Range {
            lower: Included(4),
            upper: Included(4),
        };
        assert!(exact.contains(Some(4)));
        assert!(!exact.contains(None));
        assert!(exact.may_match(
            Some(KeyRef {
                key: Some(4),
                row: 7
            }),
            Some(KeyRef {
                key: Some(4),
                row: 9
            }),
            false
        ));
        assert!(
            !Int32IndexSelection::Range {
                lower: Excluded(i32::MAX),
                upper: Unbounded
            }
            .contains(Some(i64::from(i32::MAX)))
        );
        assert!(
            !Int32IndexSelection::Range {
                lower: Included(4),
                upper: Excluded(4)
            }
            .may_match(None, None, false)
        );
        assert!(
            !Int32IndexSelection::Range {
                lower: Included(5),
                upper: Included(4)
            }
            .may_match(None, None, true)
        );
        assert!(Int32IndexSelection::Null.contains(None));
        assert!(!Int32IndexSelection::Null.contains(Some(0)));
    }
}
