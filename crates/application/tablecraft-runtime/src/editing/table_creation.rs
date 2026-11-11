//! Source-bound table creation review; no file lease is held for presentation.
mod outcome;
use super::{SaveSchemaChange, platform};
use bytes::Bytes;
pub use outcome::{TableCreationOutcomeError, TableCreationReopenFacts};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Provenance of the reviewed declaration; SQL has no table-design draft.
#[derive(Clone, Copy)]
pub(in crate::editing) enum CreationOrigin {
    Design { draft_revision: u64 },
    Sql,
}

/// Complete expected content for a fresh snapshot after an acknowledged save.
/// This grants no mutation, retry, file cleanup or physical identity authority.
pub struct CreatedTableReopen {
    pub(super) path: PathBuf,
    pub(super) expected: Bytes,
    pub(super) table: String,
    pub(super) columns: usize,
    pub(super) document_revision: u64,
    pub(super) origin: CreationOrigin,
    pub(super) schema_change: SaveSchemaChange,
}

impl CreatedTableReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn table_name(&self) -> &str {
        &self.table
    }

    pub fn column_count(&self) -> usize {
        self.columns
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    /// Actual design-draft revision, absent for a reviewed SQL declaration.
    pub fn draft_revision(&self) -> Option<u64> {
        match self.origin {
            CreationOrigin::Design { draft_revision } => Some(draft_revision),
            CreationOrigin::Sql => None,
        }
    }

    pub fn expected_schema_change(&self) -> SaveSchemaChange {
        self.schema_change
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Issued only after complete source and native declaration admission.
/// The consuming worker reconstructs the same operation against its held file.
pub struct ReviewedTableCreation {
    pub(super) state: Box<platform::table_creation::State>,
}

impl ReviewedTableCreation {
    pub fn id(&self) -> u64 {
        self.state.id
    }

    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<CreatedTableReopen> {
        Arc::clone(&self.state.reopen)
    }
}
