//! Exact original-row DELETE requests shared by normal and engineering consumers.
use super::super::rows;

/// One live ordinal in the exact captured image, with its complete raw payload.
///
/// Expected bytes start with the stored column-count header and end with the
/// final NULL/Boolean mask. They exclude the slot word and page header. They are
/// compared with the checked ordinary record, without decoding/coercing values.
/// Ordinals are snapshot-local, not persistent record identities. No Debug
/// implementation exposes record contents.
pub struct RowDelete {
    row: usize,
    expected: Vec<u8>,
}

impl RowDelete {
    /// Own an expected record; actual traversal validates its bounded framing.
    pub const fn new(row: usize, expected: Vec<u8>) -> Self {
        Self { row, expected }
    }

    /// Sequential live-row ordinal in this captured image only.
    pub const fn row(&self) -> usize {
        self.row
    }

    /// Exact expected ordinary record bytes, excluding slot/page metadata.
    pub fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

impl rows::RowAssignment for RowDelete {
    fn row(&self) -> usize {
        self.row
    }
}

/// Explicit row deletions for one table in the original captured image.
///
/// Empty groups still qualify that table. Table names may not repeat within a
/// batch, including through case-insensitive spellings. No cascade is inferred.
pub struct TableRowDeletes<'a> {
    table: &'a str,
    deletes: &'a [RowDelete],
}

impl<'a> TableRowDeletes<'a> {
    /// Borrow one table name and its snapshot-local raw row requests.
    pub const fn new(table: &'a str, deletes: &'a [RowDelete]) -> Self {
        Self { table, deletes }
    }

    /// Explicit native table name, resolved through the original catalog.
    pub const fn table(&self) -> &'a str {
        self.table
    }

    /// Exact original row requests for this table.
    pub const fn deletes(&self) -> &'a [RowDelete] {
        self.deletes
    }
}
