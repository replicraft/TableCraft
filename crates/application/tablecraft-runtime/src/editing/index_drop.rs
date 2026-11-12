//! Consuming index drop and a separate immutable read-only AFTER witness.
mod outcome;
use super::platform;
use bytes::Bytes;
pub use outcome::{IndexDropOutcomeError, IndexDropReopenFacts};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// The exact acknowledged index image, without permission to save or replay.
pub struct IndexDroppedReopen {
    pub(super) path: PathBuf,
    pub(super) expected: Bytes,
    pub(super) document_revision: u64,
    pub(super) table: String,
    pub(super) index: String,
    pub(super) native_table_id: i32,
    pub(super) definition_page: u32,
    pub(super) logical_index_number: u32,
    pub(super) physical_index_number: u32,
}

impl IndexDroppedReopen {
    /// Original source path captured by the reviewed operation.
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    /// Source document revision, not a fresh read identity.
    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    /// Exact qualified table spelling.
    pub fn table_name(&self) -> &str {
        &self.table
    }

    /// Requested index spelling.
    pub fn index_name(&self) -> &str {
        &self.index
    }

    /// Original native catalog ID.
    pub fn native_table_id(&self) -> i32 {
        self.native_table_id
    }

    /// Original native definition page.
    pub fn definition_page(&self) -> u32 {
        self.definition_page
    }

    /// Original logical index number, separate from its physical tree.
    pub fn logical_index_number(&self) -> u32 {
        self.logical_index_number
    }

    /// Original physical index number; aliases can share this identity.
    pub fn physical_index_number(&self) -> u32 {
        self.physical_index_number
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Source-issued original TCID request consumed by the single save worker.
pub struct ReviewedIndexDrop {
    pub(in crate::editing) state: Box<platform::index_drop::State>,
}

impl ReviewedIndexDrop {
    /// Issued review identity, not native target identity.
    pub fn id(&self) -> u64 {
        self.state.id
    }

    /// Original source path retained independently of UI presentation.
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    /// Share the same read-only witness without copying its database payload.
    pub fn reopen_witness(&self) -> Arc<IndexDroppedReopen> {
        Arc::clone(&self.state.reopen)
    }
}
