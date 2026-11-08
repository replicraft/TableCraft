//! Source-bound ADD-column review, separate from record edits and row counts.
use super::platform;
use crate::{Error, Response, document::Document};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Original table and proposed field names reviewed without holding a file lease.
pub struct AddColumnReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub column: String,
}

/// Complete expected content for read-only reopening after an acknowledged save.
/// It cannot authorize applying or replaying the original schema operation.
pub struct AddedColumnReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) document_revision: u64,
}

impl AddedColumnReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn table_name(&self) -> &str {
        &self.table
    }

    pub fn column_name(&self) -> &str {
        &self.column
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Issued only after native source qualification of one ordinary nullable Int32 field.
pub struct ReviewedAddColumn {
    pub(super) state: Box<platform::add_column::State>,
}

impl ReviewedAddColumn {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<AddedColumnReopen> {
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_add_int32_column(
        &mut self,
        document_revision: u64,
        table: &str,
        column: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::add_column::review(self, table, column, cancelled)
    }
}
