//! Unlocked original-field DROP and separate complete-image read witness.
use crate::{Error, Response, document::Document, editing::platform};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Exact original native field selected for removal on this document revision.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldDropReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub native_number: usize,
}

/// Complete expected result of this accepted review; no save/replay authority.
pub struct FieldDroppedReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) document_revision: u64,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) native_number: usize,
}

impl FieldDroppedReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub fn table_name(&self) -> &str {
        &self.table
    }

    pub fn original_column_name(&self) -> &str {
        &self.column
    }

    pub fn native_number(&self) -> usize {
        self.native_number
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Issued only from the actual native original-request plan, consumed once.
pub struct ReviewedFieldDrop {
    pub(in crate::editing) state: Box<platform::field_drop::State>,
}

impl ReviewedFieldDrop {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<FieldDroppedReopen> {
        Arc::clone(&self.state.reopen)
    }

    pub fn captured_table(&self) -> &str {
        self.state.reopen.table_name()
    }

    pub fn captured_column(&self) -> &str {
        self.state.reopen.original_column_name()
    }

    pub fn captured_native_number(&self) -> usize {
        self.state.reopen.native_number()
    }
}

impl Document {
    pub(crate) fn review_field_drop(
        &mut self,
        revision: u64,
        table: &str,
        column: &str,
        native_number: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::field_drop::review(self, table, column, native_number, cancelled)
    }
}
