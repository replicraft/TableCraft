//! Exact declared Decimal points, independent of SQL numeric coercion.
use super::{super::page::KeyRef, DecimalType};
use crate::InspectError;
use tablecraft_core::values::Decimal;

/// Selection through a qualified single-Decimal native index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecimalIndexSelection {
    /// Complete traversal, including NULL and complete count checks.
    All,
    /// Only stored NULL keys, distinct from every scaled zero.
    Null,
    /// Exact stored representation; scale must match and coefficient must fit field precision.
    Equal(Decimal),
}

impl DecimalIndexSelection {
    pub(in crate::table::indexes) fn validate(self, kind: DecimalType) -> Result<(), InspectError> {
        if let Self::Equal(value) = self
            && (value.scale() != kind.scale || value.coefficient().unsigned_abs() >= kind.limit())
        {
            return Err(InspectError::Unsupported);
        }
        Ok(())
    }

    fn point(self, kind: DecimalType, descending: bool) -> Option<Option<i128>> {
        match self {
            Self::All => None,
            Self::Null => Some(None),
            Self::Equal(value) => Some(Some(kind.physical_key(value.coefficient(), descending))),
        }
    }

    pub(super) fn contains(self, kind: DecimalType, descending: bool, key: Option<i128>) -> bool {
        self.point(kind, descending)
            .is_none_or(|point| point == key)
    }

    pub(super) fn may_match(
        self,
        kind: DecimalType,
        descending: bool,
        lower: Option<&KeyRef<Option<i128>>>,
        upper: Option<&KeyRef<Option<i128>>>,
    ) -> bool {
        let Some(point) = self.point(kind, descending) else {
            return true;
        };
        let (minimum, maximum) = if descending {
            (upper, lower)
        } else {
            (lower, upper)
        };
        // Fences include row references: equal scalar points may occur on either
        // side of a separator, so key-only pruning remains inclusive.
        minimum.is_none_or(|entry| entry.key <= point)
            && maximum.is_none_or(|entry| entry.key >= point)
    }
}
