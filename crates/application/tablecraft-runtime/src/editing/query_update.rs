//! Lease-free SQL UPDATE review owns the actual native plan, never UI authority.
use crate::{Error, InspectedQuery, Response, document::Document};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[path = "query_update/native.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "query_update/unsupported.rs"]
mod platform;

/// Complete reviewed counts and display facts, not permission to write.
pub struct QueryUpdateReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub matched_rows: usize,
    pub changed_rows: usize,
}

/// Runtime-issued source-bound UPDATE, consumed by the nonreplaceable save worker.
/// Its native plan and invalidation token cannot be constructed by a host.
pub struct ReviewedQueryUpdate {
    pub(super) state: Box<platform::State>,
}

pub(super) struct QueryUpdateTarget {
    pub(super) path: PathBuf,
    pub(super) document_revision: u64,
    pub(super) table: String,
}

impl ReviewedQueryUpdate {
    pub fn source_path(&self) -> &Path {
        &self.state.target.path
    }

    pub fn table(&self) -> &str {
        &self.state.target.table
    }

    pub fn document_revision(&self) -> u64 {
        self.state.target.document_revision
    }
}

impl Document {
    pub(crate) fn review_query_update(
        &mut self,
        target: &InspectedQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        platform::review(self, target, cancelled)
    }
}
