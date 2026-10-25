//! Conservative civil-order bounds over physically ordered Automation serials.
use super::super::{Int64IndexSelection, floating};
use std::{cmp::Ordering, ops::Bound};
use tablecraft_core::values::LegacyDateTime;

/// Candidate selection through a qualified single legacy DATE index.
/// Calendar bounds use day/time order, independently of physical serial order.
/// Candidates can include extra non-NULL rows; callers must apply their complete
/// temporal predicate. This does not supply SQL coercion or extended-date support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimeIndexSelection {
    /// Complete traversal, including NULL, with complete count validation.
    All,
    /// Only NULL keys; signed zero and empty dates are not NULL.
    Null,
    /// Conservative candidates for a calendar interval, excluding NULL.
    /// Boundary calendar days may be retained in full. Equal inclusive bounds
    /// use exact serial points, or a span retaining both epoch-date aliases.
    CalendarCandidates {
        /// Lower temporal endpoint; unbounded retains the earliest dates.
        lower: Bound<LegacyDateTime>,
        /// Upper temporal endpoint; unbounded retains the latest dates.
        upper: Bound<LegacyDateTime>,
    },
}

impl DateTimeIndexSelection {
    pub(in crate::table::indexes) fn keyed(self) -> Int64IndexSelection {
        let (lower, upper) = match self {
            Self::All => return Int64IndexSelection::All,
            Self::Null => return Int64IndexSelection::Null,
            Self::CalendarCandidates { lower, upper } => (lower, upper),
        };

        if empty(lower, upper) {
            return Int64IndexSelection::Range {
                lower: Bound::Included(i64::MAX),
                upper: Bound::Included(i64::MIN),
            };
        }
        if let (Bound::Included(left), Bound::Included(right)) = (lower, upper)
            && left.temporal_cmp(right).is_eq()
        {
            let serial = left.ole_days();
            let (minimum, maximum) = if left.day_offset() == 0 {
                (-serial.abs(), serial.abs())
            } else {
                (serial, serial)
            };
            return Int64IndexSelection::Range {
                lower: Bound::Included(key(minimum)),
                upper: Bound::Included(key(maximum)),
            };
        }

        // Negative-day time fractions run backwards in physical serial order.
        // Retaining each boundary day in full avoids dropping temporal matches.
        // Epoch day additionally occupies both negative and positive fractions.
        Int64IndexSelection::Range {
            lower: match lower {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(value) | Bound::Excluded(value) => {
                    let day = value.day_offset();
                    let serial = if day <= 0 {
                        (f64::from(day) - 1.0).next_up()
                    } else {
                        f64::from(day)
                    };
                    Bound::Included(key(serial))
                }
            },
            upper: match upper {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(value) | Bound::Excluded(value) => {
                    let day = value.day_offset();
                    let serial = if day < 0 {
                        f64::from(day)
                    } else {
                        (f64::from(day) + 1.0).next_down()
                    };
                    Bound::Included(key(serial))
                }
            },
        }
    }
}

fn key(serial: f64) -> i64 {
    floating::double_key(serial.to_bits())
}

fn empty(lower: Bound<LegacyDateTime>, upper: Bound<LegacyDateTime>) -> bool {
    match (lower, upper) {
        (Bound::Unbounded, _) | (_, Bound::Unbounded) => false,
        (
            Bound::Included(left) | Bound::Excluded(left),
            Bound::Included(right) | Bound::Excluded(right),
        ) => match left.temporal_cmp(right) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => {
                matches!(lower, Bound::Excluded(_)) || matches!(upper, Bound::Excluded(_))
            }
        },
    }
}

#[cfg(test)]
mod tests;
