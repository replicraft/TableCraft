//! A review binds original table identity; its reopen witness cannot save or replay.
use crate::{Error, Response, document::Document, editing::platform};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Exact source facts issued with one consuming native DROP capability.
#[derive(Debug, PartialEq, Eq)]
pub struct TableDropReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub native_id: i32,
    pub declared_rows: u32,
}

/// Complete expected AFTER for this accepted table DROP, without mutation authority.
pub struct TableDroppedReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) document_revision: u64,
    pub(crate) table: String,
    pub(crate) native_id: i32,
}

impl TableDroppedReopen {
    /// Exact original native source path, independent of displayed text.
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    /// Original document revision accepted by the review.
    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    /// True native name of the removed table.
    pub fn table_name(&self) -> &str {
        &self.table
    }

    /// Original persistent native catalog ID.
    pub fn native_id(&self) -> i32 {
        self.native_id
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// A sealed original-request plan consumed by the single save worker.
pub struct ReviewedTableDrop {
    pub(in crate::editing) state: Box<platform::table_drop::State>,
}

impl ReviewedTableDrop {
    /// Exact source path retained by the native review.
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    /// Obtain the same read-only complete-image witness, never another save owner.
    pub fn reopen_witness(&self) -> Arc<TableDroppedReopen> {
        // NO_CLONE: The consuming save and later UI reopen need the same immutable witness.
        // Arc shares its checked image across threads; a borrowed witness could not outlive save.
        Arc::clone(&self.state.reopen)
    }

    /// Exact original true name, retained independently of caller intent.
    pub fn captured_table(&self) -> &str {
        self.state.reopen.table_name()
    }

    /// Original persistent catalog identity retained by the native source proof.
    pub fn captured_native_id(&self) -> i32 {
        self.state.reopen.native_id()
    }

    /// Original document revision captured by this review.
    pub fn captured_document_revision(&self) -> u64 {
        self.state.reopen.document_revision()
    }
}

impl Document {
    pub(crate) fn review_table_drop(
        &mut self,
        revision: u64,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::table_drop::review(self, table, cancelled)
    }
}
