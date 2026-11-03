//! Owned selected-parent storage, borrowing the actual typed attachment view.
use super::super::{StoredAttachment, record, schema::Columns};
use crate::{InspectError, NativeRowReference};
use tablecraft_core::values::Value;

/// One complete parent's stored attachment list, not a whole-column snapshot.
/// Metadata and encoded FileData remain owned after the issuing reader drops.
/// Unknown wrappers stay inert; StoredAttachment::decode_with is explicit.
pub struct ParentAttachments {
    pub(super) reference: NativeRowReference,
    pub(super) key: Option<i32>,
    pub(super) rows: Vec<Vec<Value>>,
    pub(super) columns: Columns,
}

impl ParentAttachments {
    /// Original logical parent root, scoped to the issuing source and schema.
    pub const fn row_reference(&self) -> NativeRowReference {
        self.reference
    }

    /// Raw parent complex key; NULL differs from an assigned empty collection.
    pub const fn key(&self) -> Option<i32> {
        self.key
    }

    /// Complete stored child order, including children whose FileData is NULL.
    /// This uses the same typed decoder as the existing full attachment reader.
    pub fn attachments(
        &self,
    ) -> impl ExactSizeIterator<Item = Result<StoredAttachment<'_>, InspectError>> {
        self.rows.iter().map(|row| record::read(row, &self.columns))
    }
}
