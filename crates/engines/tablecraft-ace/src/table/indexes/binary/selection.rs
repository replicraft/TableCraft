//! Borrowed logical byte bounds and a conservative contiguous physical hull.
use super::*;
use std::ops::Bound;

const MINIMUM_NONEMPTY: &[u8] = &[0];
const MAXIMUM_BYTES: [u8; MAX_SHORT_BINARY_BYTES] = [u8::MAX; MAX_SHORT_BINARY_BYTES];

/// Exact Short Binary selection, without text conversion or SQL coercion.
/// Range endpoints use ascending byte order regardless of index direction and
/// exclude NULL. Reversed/empty intervals select no rows. Bounds are borrowed;
/// endpoints longer than the native carrier ceiling fail before traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryIndexSelection<'a> {
    /// Complete traversal including NULL, with complete count checks.
    All,
    /// Only NULL keys.
    Null,
    /// Inclusive/exclusive/unbounded raw byte interval, excluding NULL.
    Range {
        /// Lower endpoint in ordinary lexicographic byte order.
        lower: Bound<&'a [u8]>,
        /// Upper endpoint in ordinary lexicographic byte order.
        upper: Bound<&'a [u8]>,
    },
}
impl BinaryIndexSelection<'_> {
    /// Maximum byte length of each borrowed endpoint, independent of field length.
    pub const MAX_BOUND_BYTES: usize = MAX_SHORT_BINARY_BYTES;
    pub(in crate::table::indexes) fn validate(self) -> Result<(), InspectError> {
        if let Self::Range { lower, upper } = self {
            for bound in [lower, upper] {
                if matches!(bound, Bound::Included(value) | Bound::Excluded(value) if value.len() > MAX_SHORT_BINARY_BYTES)
                {
                    return Err(InspectError::StructureLimit);
                }
            }
        }
        Ok(())
    }
    pub(super) fn contains(self, key: Option<BinaryKey>) -> bool {
        match (self, key) {
            (Self::All, _) | (Self::Null, None) => true,
            (Self::Range { lower, upper }, Some(value)) => {
                !below(value.bytes(), lower) && !above(value.bytes(), upper)
            }
            _ => false,
        }
    }
    fn empty(self) -> bool {
        use Bound::*;
        match self {
            Self::Range {
                lower: Included(a),
                upper: Included(b),
            } => a > b,
            Self::Range {
                lower: Included(a) | Excluded(a),
                upper: Included(b) | Excluded(b),
            } => a >= b,
            _ => false,
        }
    }
    pub(super) fn may_match(
        self,
        lower: Option<super::super::page::KeyRef<Option<BinaryKey>>>,
        upper: Option<super::super::page::KeyRef<Option<BinaryKey>>>,
        descending: bool,
        maximum: usize,
    ) -> bool {
        if self.empty() {
            return false;
        }
        let (first, last) = match self {
            Self::All => return true,
            Self::Null => (None, None),
            Self::Range { lower, upper } => {
                let include_empty = !below(&[], lower) && !above(&[], upper);
                let smallest = match lower {
                    Bound::Included(value) | Bound::Excluded(value) if !value.is_empty() => value,
                    _ => MINIMUM_NONEMPTY,
                };
                let largest = match upper {
                    Bound::Included(value) | Bound::Excluded(value) => value,
                    Bound::Unbounded => &MAXIMUM_BYTES[..maximum],
                };
                if smallest > largest {
                    if !include_empty {
                        return false;
                    }
                    (Some(&[][..]), Some(&[][..]))
                } else {
                    let (first, last) = if descending {
                        (largest, smallest)
                    } else {
                        (smallest, largest)
                    };
                    // Empty is always the first non-NULL physical value. Taking
                    // its hull with the nonempty interval keeps traversal contiguous.
                    (
                        Some(if include_empty { &[][..] } else { first }),
                        Some(last),
                    )
                }
            }
        };
        !lower.is_some_and(|bound| {
            compare_bytes(bound.key.as_ref().map(BinaryKey::bytes), last, descending).is_gt()
        }) && !upper.is_some_and(|bound| {
            compare_bytes(bound.key.as_ref().map(BinaryKey::bytes), first, descending).is_lt()
        })
    }
}
fn below(value: &[u8], lower: Bound<&[u8]>) -> bool {
    match lower {
        Bound::Included(bound) => value < bound,
        Bound::Excluded(bound) => value <= bound,
        Bound::Unbounded => false,
    }
}
fn above(value: &[u8], upper: Bound<&[u8]>) -> bool {
    match upper {
        Bound::Included(bound) => value > bound,
        Bound::Excluded(bound) => value >= bound,
        Bound::Unbounded => false,
    }
}
