//! Explicit reviewed database settings, independent of table and record edits.
use super::{SaveDatabaseOptionsChange, platform};
use crate::{Error, Response, document::Document};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// The explicit requested change and source revision, never save authority.
pub struct DatabaseOptionsReview {
    pub id: u64,
    pub document_revision: u64,
    pub change: SaveDatabaseOptionsChange,
}

/// Read-only evidence for a fresh open after an acknowledged option save.
/// Possession of this witness cannot execute or replay the operation.
pub struct DatabaseOptionsReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) document_revision: u64,
}

impl DatabaseOptionsReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Only a successful explicit runtime review issues this consuming capability.
pub struct ReviewedDatabaseOptions {
    pub(super) state: Box<platform::database_options::State>,
}

impl ReviewedDatabaseOptions {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<DatabaseOptionsReopen> {
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_disable_name_auto_correct(
        &mut self,
        document_revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::database_options::review(self, cancelled)
    }
}
