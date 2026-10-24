use crate::{
    InspectError, NativeColumn, NativeSortOrder,
    collation::equality_primary_prefix,
    format::data_type::{MAX_SHORT_TEXT_UNITS, SHORT_TEXT},
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

/// Owned primary prefix for conservative Short Text General equality candidates.
/// This retains exact native sort identity, not a full equality result. Every
/// candidate still requires the original complete comparison under that policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEqualityPrefix {
    order: NativeSortOrder,
    length: u8,
    bytes: [u8; Self::MAX_BYTES],
}

impl GeneralEqualityPrefix {
    /// Initial primary bytes retained strictly before physical entry digests.
    pub const MAX_BYTES: usize = 32;

    /// Derive a prefix under the explicit complete General equality model.
    /// Literal input is bounded to 255 UTF-16 units; budgets stay cumulative.
    pub fn from_text(
        order: NativeSortOrder,
        literal: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.text.equality_prefix").entered();
        check(budget, cancelled)?;
        for (position, _) in literal.encode_utf16().enumerate() {
            check(budget, cancelled)?;
            if position >= MAX_SHORT_TEXT_UNITS {
                return Err(InspectError::StructureLimit);
            }
        }
        let prefix = equality_primary_prefix(order, literal, Self::MAX_BYTES, budget, cancelled)?;
        Self::from_encoded(order, &prefix)
    }

    /// Recover a bounded prefix from a trusted same-policy host hint.
    /// Validation does not prove that arbitrary supplied bytes describe a literal;
    /// the host owns provenance and complete-comparer agreement for these bytes.
    pub fn from_encoded(order: NativeSortOrder, bytes: &[u8]) -> Result<Self, InspectError> {
        if order.general_family().is_none() {
            return Err(InspectError::Unsupported);
        }
        if bytes.len() > Self::MAX_BYTES {
            return Err(InspectError::StructureLimit);
        }

        let mut prefix = Self {
            order,
            length: bytes.len() as u8,
            bytes: [0; Self::MAX_BYTES],
        };
        prefix.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(prefix)
    }

    /// Exact descriptor that qualified prefix generation.
    pub const fn order(self) -> NativeSortOrder {
        self.order
    }

    /// Initial primary bytes, excluding field markers and direction inversion.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }

    /// Metadata-only eligibility; Memo and different/unknown orders are excluded.
    pub fn supports_column(self, column: &NativeColumn) -> bool {
        let width = size_of::<u16>();
        column.native_type() == SHORT_TEXT
            && column.text_sort_order() == Some(self.order)
            && column.storage_length().is_multiple_of(width)
            && column.storage_length() / width <= MAX_SHORT_TEXT_UNITS
    }
}

#[cfg(test)]
mod tests;
