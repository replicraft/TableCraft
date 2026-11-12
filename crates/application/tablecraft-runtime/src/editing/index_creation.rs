//! Consuming index creation and a separate immutable read-only AFTER witness.
mod outcome;
use super::platform;
use bytes::Bytes;
pub use outcome::{IndexCreationOutcomeError, IndexCreationReopenFacts};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// The exact acknowledged index image, without permission to save or replay.
pub struct IndexCreatedReopen {
    pub(super) path: PathBuf,
    pub(super) expected: Bytes,
    pub(super) document_revision: u64,
    pub(super) table: String,
    pub(super) index: String,
    pub(super) column: String,
    pub(super) native_table_id: i32,
    pub(super) definition_page: u32,
    pub(super) native_number: u16,
    pub(super) column_id: u16,
    pub(super) descending: bool,
    pub(super) unique: bool,
    pub(super) indexed_rows: usize,
}

impl IndexCreatedReopen {
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
    /// Actual source field name; no display ordinal selects the target.
    pub fn column_name(&self) -> &str {
        &self.column
    }
    /// Original native catalog ID.
    pub fn native_table_id(&self) -> i32 {
        self.native_table_id
    }
    /// Original native definition page.
    pub fn definition_page(&self) -> u32 {
        self.definition_page
    }
    /// Original native field number, including ordinary field zero.
    pub fn native_number(&self) -> u16 {
        self.native_number
    }
    /// Original stored field ID, separate from native number.
    pub fn column_id(&self) -> u16 {
        self.column_id
    }
    /// Original requested key direction.
    pub fn descending(&self) -> bool {
        self.descending
    }
    /// Whether the exact reviewed native plan enforces non-NULL uniqueness.
    pub fn unique(&self) -> bool {
        self.unique
    }

    /// Existing source population, never an affected-row receipt.
    pub fn indexed_rows(&self) -> usize {
        self.indexed_rows
    }
    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Source-issued original TCIC request consumed by the single save worker.
pub struct ReviewedIndexCreation {
    pub(in crate::editing) state: Box<platform::index_creation::State>,
}

impl ReviewedIndexCreation {
    /// Issued review identity, not native target identity.
    pub fn id(&self) -> u64 {
        self.state.id
    }
    /// Original source path retained independently of UI presentation.
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }
    /// Share the same read-only witness without copying its database payload.
    pub fn reopen_witness(&self) -> Arc<IndexCreatedReopen> {
        Arc::clone(&self.state.reopen)
    }
}
