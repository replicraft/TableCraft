//! Owned selected-parent rows with the existing borrowed StoredVersion view.
use super::super::{StoredVersion, record, schema::Columns};
use crate::{InspectError, NativeRowReference};
use tablecraft_core::values::Value;

/// One parent's complete stored history, separate from current visible Memo data.
/// Original order, nullable Text and exact stored DATE bits remain unchanged.
/// Owned rows can outlive the reader; ROW/key identity stays source/schema-local.
pub struct ParentVersionHistory {
    pub(super) reference: NativeRowReference,
    pub(super) key: Option<i32>,
    pub(super) rows: Vec<Vec<Value>>,
    pub(super) columns: Columns,
}

impl ParentVersionHistory {
    /// Original logical parent root, not a forwarded terminal storage address.
    pub const fn row_reference(&self) -> NativeRowReference {
        self.reference
    }

    /// Raw hidden history key; NULL differs from an assigned empty history list.
    pub const fn key(&self) -> Option<i32> {
        self.key
    }

    /// Stored versions in flat-root order, without sorting or latest selection.
    pub fn stored_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = Result<StoredVersion<'_>, InspectError>> {
        self.rows.iter().map(|row| record::read(row, &self.columns))
    }
}
