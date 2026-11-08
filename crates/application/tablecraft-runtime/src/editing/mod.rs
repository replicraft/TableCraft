//! Explicit normal native editing; drafts never hold an offline file lease.
#[cfg(feature = "ace16-create")]
mod add_column;
mod deleting;
mod inserting;
mod query_action;
mod query_delete;
mod query_expression;
mod query_insert;
mod query_sql;
mod query_update;
mod replacing;
mod save;
#[cfg(feature = "ace16-create")]
mod table_creation;
mod table_rename;

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[path = "platform/native/mod.rs"]
pub(crate) mod platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "platform/unsupported/mod.rs"]
pub(crate) mod platform;

#[cfg(feature = "ace16-create")]
pub use add_column::{AddColumnReview, AddedColumnReopen, ReviewedAddColumn};
pub use deleting::{DeleteReview, DeleteTarget, ReviewedDelete};
pub use inserting::{
    InsertDraft, InsertField, InsertInput, InsertRetryEvidence, InsertRetryWitness, InsertReview,
    InsertTarget, ReviewedInsert,
};
pub use platform::save::SaveWorker;
pub use query_delete::{QueryDeleteReview, ReviewedQueryDelete};
pub use query_expression::{QueryExpressionReopen, QueryExpressionReview, ReviewedQueryExpression};
pub use query_insert::{QueryInsertReview, ReviewedQueryInsert};
pub use query_sql::{QuerySqlDraft, QuerySqlEditError, QuerySqlEligibility, QuerySqlReview};
pub use query_update::{QueryUpdateReview, ReviewedQueryUpdate};
pub use replacing::ReplaceError;
pub use save::{
    SaveCleanupDisposition, SaveCompletion, SaveDatabaseOptionsChange, SaveFailure,
    SaveFailureKind, SaveKnownOutcome, SaveOperation, SaveOutcome, SavePhase, SaveQueryChange,
    SaveReceipt, SaveReport, SaveRetirementGuarantee, SaveSchemaChange, SaveSessionState,
    SaveStartError, SaveStartFailure,
};
#[cfg(feature = "ace16-create")]
pub use table_creation::{CreatedTableReopen, ReviewedTableCreation};
pub use table_rename::{ReviewedTableRename, TableRenameReopen, TableRenameReview};

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
    state: Box<platform::state::ReviewedState>,
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
