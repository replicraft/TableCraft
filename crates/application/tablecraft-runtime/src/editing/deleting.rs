//! Source-bound single-row deletion review without an offline lease.
use super::platform;
use crate::{Cell, Column};
use std::path::Path;

#[derive(Clone, PartialEq, Eq)]
pub struct DeleteTarget {
    pub document_revision: u64,
    pub table: String,
    pub parent_ordinal: u32,
}

pub struct DeleteReview {
    pub id: u64,
    pub target: DeleteTarget,
    pub columns: Vec<Column>,
    pub row: Vec<Cell>,
    pub deleted_rows: usize,
}

pub struct ReviewedDelete {
    pub(super) state: Box<platform::deleting::State>,
}

impl ReviewedDelete {
    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn target(&self) -> &DeleteTarget {
        &self.state.target
    }

    /// A display position in the remaining full relation, never a native handle.
    pub fn refresh_ordinal(&self) -> u32 {
        self.state.refresh_ordinal
    }
}
