//! Source-bound catalog rename review, separate from record edits and counts.
use super::platform;
use crate::{Error, Response, document::Document};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Names and source revision reviewed by the user, never mutation authority.
pub struct TableRenameReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub new_name: String,
    pub changed: bool,
}

/// Expected content for a later read-only reopen after an acknowledged save.
/// This witness cannot save or authorize replaying the rename.
pub struct TableRenameReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) table: String,
    pub(crate) new_name: String,
    pub(crate) document_revision: u64,
}

impl TableRenameReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn original_name(&self) -> &str {
        &self.table
    }

    pub fn table_name(&self) -> &str {
        &self.new_name
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Only a successful runtime review constructs this consuming capability.
pub struct ReviewedTableRename {
    pub(super) state: Box<platform::table_rename::State>,
}

impl ReviewedTableRename {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<TableRenameReopen> {
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_table_rename(
        &mut self,
        document_revision: u64,
        table: &str,
        new_name: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::table_rename::review(self, table, new_name, cancelled)
    }
}
