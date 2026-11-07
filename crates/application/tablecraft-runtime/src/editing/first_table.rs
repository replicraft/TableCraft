//! Source-bound initial-table review; no file lease is held for presentation.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod native;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use crate::{Error, Response, document::Document};
use bytes::Bytes;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::sync::atomic::AtomicBool;
use std::{path::Path, path::PathBuf, sync::Arc};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_ace::FirstTableDefinition;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use tablecraft_builders::table::TableDraftSnapshot;

/// Complete expected content for a fresh snapshot after an acknowledged save.
/// This grants no mutation, retry, file cleanup or physical identity authority.
pub struct FirstTableReopen {
    pub(super) path: PathBuf,
    pub(super) expected: Bytes,
    pub(super) table: String,
    pub(super) columns: usize,
    pub(super) document_revision: u64,
    pub(super) draft_revision: u64,
}

impl FirstTableReopen {
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

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Issued only after complete source and native declaration admission.
/// The consuming worker reconstructs the same operation against its held file.
pub struct ReviewedFirstTable {
    pub(super) state: Box<ReviewedFirstTableState>,
}

pub(super) struct ReviewedFirstTableState {
    pub(super) id: u64,
    pub(super) reopen: Arc<FirstTableReopen>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) original: Bytes,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) definition: FirstTableDefinition,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) valid: Arc<AtomicBool>,
}

impl ReviewedFirstTable {
    pub fn id(&self) -> u64 {
        self.state.id
    }

    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<FirstTableReopen> {
        Arc::clone(&self.state.reopen)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl Document {
    pub(crate) fn review_first_table(
        &mut self,
        _document_revision: u64,
        _snapshot: &TableDraftSnapshot,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(super::EditError::UnsupportedPlatform.into())
    }
}
