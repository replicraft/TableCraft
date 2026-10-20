//! Explicit physical cursor intervals, not SQL string predicates.
use super::{KeyWork, TextKey, TextProfile};
use crate::{InspectError, format::data_type::MAX_SHORT_TEXT_UNITS, header::check};
use std::ops::Bound;
use tablecraft_core::limits::ResourceBudget;

/// Selection through a mandatory single General Short Text index.
/// Range endpoints are encoded using that column's sort descriptor/direction.
/// Start/end follow physical index order: a descending interval may start at
/// "z" and end at "a". Native key equivalence includes case/ignored weights and
/// overflow digests; it is not full-string SQL comparison. No coercion occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextIndexSelection<'a> {
    /// Complete traversal including NULL, retaining complete count checks.
    All,
    /// Only NULL keys, distinct from empty or ignored text.
    Null,
    /// Inclusive/exclusive/unbounded physical interval, excluding NULL.
    /// Reversed or empty encoded intervals select no rows.
    Range {
        /// First physical key; descending uses the larger text endpoint first.
        start: Bound<&'a str>,
        /// Last physical key in the selected index direction.
        end: Bound<&'a str>,
    },
}
impl TextIndexSelection<'_> {
    /// Maximum UTF-16 units per endpoint, independent of declared field width.
    pub const MAX_BOUND_UNITS: usize = MAX_SHORT_TEXT_UNITS;
    pub(in crate::table::indexes) fn validate(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        if let Self::Range { start, end } = self {
            for bound in [start, end] {
                if let Bound::Included(text) | Bound::Excluded(text) = bound {
                    for (position, _) in text.encode_utf16().enumerate() {
                        check(budget, cancelled)?;
                        if position >= MAX_SHORT_TEXT_UNITS {
                            return Err(InspectError::StructureLimit);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) enum EncodedSelection {
    All,
    Null,
    Range {
        start: Bound<TextKey>,
        end: Bound<TextKey>,
    },
}
impl EncodedSelection {
    pub fn new(
        selection: TextIndexSelection<'_>,
        profile: &TextProfile,
        work: &mut KeyWork<'_>,
    ) -> Result<Self, InspectError> {
        fn encode(
            bound: Bound<&str>,
            profile: &TextProfile,
            work: &mut KeyWork<'_>,
        ) -> Result<Bound<TextKey>, InspectError> {
            Ok(match bound {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(text) => {
                    Bound::Included(profile.encode(text, MAX_SHORT_TEXT_UNITS, work)?)
                }
                Bound::Excluded(text) => {
                    Bound::Excluded(profile.encode(text, MAX_SHORT_TEXT_UNITS, work)?)
                }
            })
        }
        Ok(match selection {
            TextIndexSelection::All => Self::All,
            TextIndexSelection::Null => Self::Null,
            TextIndexSelection::Range { start, end } => Self::Range {
                start: encode(start, profile, work)?,
                end: encode(end, profile, work)?,
            },
        })
    }
    pub fn contains(&self, bytes: &[u8], null: u8) -> bool {
        match self {
            Self::All => true,
            Self::Null => bytes == [null],
            Self::Range { start, end } => {
                bytes != [null]
                    && match start {
                        Bound::Unbounded => true,
                        Bound::Included(key) => bytes >= key.0.as_slice(),
                        Bound::Excluded(key) => bytes > key.0.as_slice(),
                    }
                    && match end {
                        Bound::Unbounded => true,
                        Bound::Included(key) => bytes <= key.0.as_slice(),
                        Bound::Excluded(key) => bytes < key.0.as_slice(),
                    }
            }
        }
    }
    pub fn may_match(&self, lower: Option<&[u8]>, upper: Option<&[u8]>, null: u8) -> bool {
        let null_key = [null];
        let (first, last) = match self {
            Self::All => return true,
            Self::Null => (Some(null_key.as_slice()), Some(null_key.as_slice())),
            Self::Range { start, end } => {
                fn bound(b: &Bound<TextKey>) -> Option<&[u8]> {
                    match b {
                        Bound::Unbounded => None,
                        Bound::Included(key) | Bound::Excluded(key) => Some(key.0.as_slice()),
                    }
                }
                let first = bound(start);
                let last = bound(end);
                if first.zip(last).is_some_and(|(a, b)| {
                    a > b
                        || a == b
                            && (matches!(start, Bound::Excluded(_))
                                || matches!(end, Bound::Excluded(_)))
                }) {
                    return false;
                }
                (first, last)
            }
        };
        // Coarse inclusive fences retain duplicate endpoint keys on both sides
        // of a row-reference separator. Exclusive endpoints are filtered at leaves.
        !lower.zip(last).is_some_and(|(a, b)| a > b)
            && !upper.zip(first).is_some_and(|(a, b)| a < b)
    }
}
