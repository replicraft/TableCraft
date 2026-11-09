//! Unlocked original-name review and separate complete-image read witness.
use crate::{Error, Response, document::Document, editing::platform};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Exact native field and spelling selected on this document revision.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldRenameReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub native_number: usize,
    pub new_name: String,
    pub changed: bool,
}

/// Complete expected result of this accepted review; no save/replay authority.
pub struct FieldRenamedReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) document_revision: u64,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) native_number: usize,
    pub(crate) new_name: String,
    pub(crate) changed: bool,
}

impl FieldRenamedReopen {
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

    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Issued only from the actual native original-request plan, consumed once.
pub struct ReviewedFieldRename {
    pub(in crate::editing) state: Box<platform::field_rename::State>,
}

impl ReviewedFieldRename {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<FieldRenamedReopen> {
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

    pub fn captured_new_name(&self) -> &str {
        self.state.reopen.new_name()
    }

    pub fn changed(&self) -> bool {
        self.state.reopen.changed()
    }

    /// Consume an unchanged native review without starting a save actor or
    /// creating a journal. Only the complete read-only witness remains.
    pub fn into_unchanged_reopen(self) -> Result<Arc<FieldRenamedReopen>, Self> {
        if self.changed() || !self.state.valid.load(std::sync::atomic::Ordering::Acquire) {
            return Err(self);
        }
        let state = *self.state;
        Ok(state.reopen)
    }
}

impl Document {
    pub(crate) fn review_field_rename(
        &mut self,
        revision: u64,
        table: &str,
        column: &str,
        native_number: usize,
        new_name: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::field_rename::review(self, table, column, native_number, new_name, cancelled)
    }
}
