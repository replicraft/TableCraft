//! Source-bound table creation review; no file lease is held for presentation.
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod native;

use super::SaveSchemaChange;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use crate::{Error, Response, document::Document};
use bytes::Bytes;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use std::sync::atomic::AtomicBool;
use std::{path::Path, path::PathBuf, sync::Arc};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_ace::FirstTableDefinition;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use tablecraft_builders::table::TableDraftSnapshot;

/// Complete expected content for a fresh snapshot after an acknowledged save.
/// This grants no mutation, retry, file cleanup or physical identity authority.
pub struct CreatedTableReopen {
    pub(super) path: PathBuf,
    pub(super) expected: Bytes,
    pub(super) table: String,
    pub(super) columns: usize,
    pub(super) document_revision: u64,
    pub(super) draft_revision: u64,
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

    pub fn draft_revision(&self) -> u64 {
        self.draft_revision
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
    pub(super) state: Box<ReviewedTableCreationState>,
}

pub(super) struct ReviewedTableCreationState {
    pub(super) id: u64,
    pub(super) reopen: Arc<CreatedTableReopen>,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) original: Bytes,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) definition: FirstTableDefinition,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) valid: Arc<AtomicBool>,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) profile: TableCreationProfile,
}

/// Captured from the current document's source qualification, never caller input.
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[derive(Clone, Copy)]
pub(crate) enum TableCreationProfile {
    First,
    Added,
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
impl TableCreationProfile {
    pub(super) fn schema_change(self, column_count: usize) -> SaveSchemaChange {
        match self {
            Self::First => SaveSchemaChange::FirstTableCreated { column_count },
            Self::Added => SaveSchemaChange::TableAdded { column_count },
        }
    }
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

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Document {
    pub(crate) fn review_table_creation(
        &mut self,
        _document_revision: u64,
        _snapshot: &TableDraftSnapshot,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(super::EditError::UnsupportedPlatform.into())
    }
}
