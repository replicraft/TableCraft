//! Explicit normal native editing; drafts never hold an offline file lease.
mod deleting;
mod query_update;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[path = "state/native.rs"]
pub(crate) mod state;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "state/unsupported.rs"]
pub(crate) mod state;
pub use query_update::{QueryUpdateReview, ReviewedQueryUpdate};
#[cfg(feature = "ace16-create")]
mod table_creation;
#[cfg(all(
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
pub(crate) use table_creation::TableCreationProfile;
#[cfg(feature = "ace16-create")]
pub use table_creation::{CreatedTableReopen, ReviewedTableCreation};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod draft;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod ids;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod input;
mod inserting;
mod replacing;
pub use deleting::{DeleteReview, DeleteTarget, ReviewedDelete};
pub use replacing::ReplaceError;
mod save;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod unsupported;

pub use inserting::{
    InsertDraft, InsertField, InsertInput, InsertRetryEvidence, InsertRetryWitness, InsertReview,
    InsertTarget, ReviewedInsert,
};
pub use save::{
    SaveCleanupDisposition, SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome,
    SaveOperation, SaveOutcome, SavePhase, SaveReceipt, SaveReport, SaveRetirementGuarantee,
    SaveSchemaChange, SaveSessionState, SaveStartError, SaveStartFailure, SaveWorker,
};

use crate::{Cell, scalar_input::InputHint};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Clone, PartialEq, Eq)]
pub struct EditTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub parent_ordinal: u32,
}

#[derive(PartialEq, Eq)]
pub struct EditInput {
    pub text: String,
    pub is_null: bool,
}

pub struct EditDraft {
    pub id: u64,
    pub target: EditTarget,
    pub original: Cell,
    pub initial: EditInput,
    pub input_hint: InputHint,
    /// Reader nullability only; Required and all other backend gates still apply.
    pub nullable: bool,
}

pub struct EditReview {
    pub id: u64,
    pub draft_id: u64,
    pub target: EditTarget,
    pub before: Cell,
    pub after: Cell,
    pub changed_rows: usize,
}

/// A real normal native plan. Only a successful source-bound review can issue it.
/// It is consumed by SaveWorker, never fabricated from a presentation DTO.
pub struct ReviewedEdit {
    state: Box<state::ReviewedState>,
}

/// Reserved durable sibling used by the desktop host and restart guard.
/// Native path bytes are retained; this does not create or inspect the entry.
pub fn journal_path(source: &Path) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(source.file_name().unwrap_or(source.as_os_str()));
    name.push(".tablecraft-journal");
    source.with_file_name(name)
}

impl ReviewedEdit {
    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn target(&self) -> &EditTarget {
        &self.state.target
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("the viewed source is not a standalone unencrypted native file")]
    ReadOnlySource,
    #[error("the edit no longer belongs to the current draft or document")]
    StaleDraft,
    #[error("the original row or column was not found")]
    TargetMissing,
    #[error("no ordinary native editing service is available for this table")]
    Unavailable,
    #[error("the exact replacement value is invalid for this field")]
    InvalidInput,
    #[error("the replacement input exceeds the editing limit")]
    InputLimit,
    #[error("this declared carrier has no supported input conversion")]
    UnsupportedInput,
    #[error(transparent)]
    Editor(#[from] tablecraft_access::editing::NativeEditError),
    #[error(transparent)]
    Plan(#[from] tablecraft_access::editing::journal::Error),
    #[error("offline editing is not supported on this platform")]
    UnsupportedPlatform,
}
