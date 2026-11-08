//! One reviewed row of original presence-bearing native INSERT intent.
mod retry;
use super::EditInput;
use crate::scalar_input::InputHint;
pub use retry::{InsertRetryEvidence, InsertRetryWitness};
use std::{path::Path, sync::Arc};

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[path = "inserting/native.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "inserting/unsupported.rs"]
mod platform;

#[derive(Clone, PartialEq, Eq)]
pub struct InsertTarget {
    pub document_revision: u64,
    pub table: String,
}

#[derive(PartialEq, Eq)]
pub enum InsertInput {
    Omitted,
    Value(EditInput),
}

pub struct InsertField {
    pub name: String,
    pub input_hint: InputHint,
    /// Reader nullability does not establish native Required/default semantics.
    pub nullable: bool,
    /// Actual native AutoNumber/replication-ID marker. This does not select a
    /// generation mode or grant native insertion permission; input stays omitted.
    pub auto_number: bool,
}

pub struct InsertDraft {
    pub id: u64,
    pub target: InsertTarget,
    pub fields: Vec<InsertField>,
}

pub struct InsertReview {
    pub id: u64,
    pub draft_id: u64,
    pub target: InsertTarget,
    /// Exact user intent; omissions are not substituted with generated values.
    pub inputs: Vec<InsertInput>,
    pub inserted_rows: usize,
}

pub struct ReviewedInsert {
    pub(super) state: Box<platform::State>,
}

impl ReviewedInsert {
    /// Borrow read-only original content evidence before consuming the review.
    /// This grants no retry permission or automatic replay.
    pub fn retry_witness(&self) -> Arc<InsertRetryWitness> {
        Arc::clone(&self.state.retry)
    }

    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn target(&self) -> &InsertTarget {
        &self.state.target
    }
}
