//! Shared application sessions and cancellable native read tasks for hosts.
mod cell;
mod column;
mod contents;
mod controls;
mod display;
mod document;
#[cfg(feature = "offline-edit")]
mod editing;
mod native_properties;
mod query;
mod rows;
mod saved;
mod scalar_input;
mod schema;
mod source;
mod worker;

pub use cell::{
    Cell, CellKind, DateRepresentation, DisplayControl, HorizontalAlignment, NativeFormat,
    ScalarPresentation,
};
pub use column::Column;
pub use contents::{CollectionKind, ContentsTarget};
#[cfg(feature = "offline-edit")]
pub use editing::{
    DeleteReview, DeleteTarget, EditDraft, EditError, EditInput, EditReview, EditTarget, InputHint,
    InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget, ReviewedDelete,
    ReviewedEdit, ReviewedInsert, SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome,
    SaveOperation, SaveOutcome, SavePhase, SaveReceipt, SaveReport, SaveSessionState,
    SaveStartError, SaveStartFailure, SaveWorker, journal_path,
};
pub use saved::{
    MAX_SAVED_QUERY_INPUT_BYTES, MAX_SAVED_QUERY_PARAMETERS, ParameterInput, ParameterSpec,
    SavedQuery, SavedQueryTarget,
};
pub use schema::{Field, FieldProperty, FieldPropertyValue};
pub use worker::Worker;

use std::path::PathBuf;
use zeroize::Zeroize;

/// A typed task failure. Display text contains no credentials or row payloads.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "offline-edit")]
    #[error("a reserved journal entry requires explicit recovery or diagnosis before opening")]
    RetainedJournal,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error(transparent)]
    Conversion(#[from] tablecraft_expressions::EvalError),
    #[error("typed input is invalid")]
    InvalidScalarInput,
    #[error("typed input exceeds the input limit")]
    ScalarInputLimit,
    #[error("saved query target belongs to another document or object")]
    StaleSavedQuery,
    #[error("saved query parameter inputs do not match their declarations")]
    ParameterInput,
    #[error(transparent)]
    Saved(#[from] tablecraft_access::NativeSavedReadError),
    #[error("operation cancelled")]
    Cancelled,
    #[error("no database is open")]
    NoDatabase,
    #[error("the collection belongs to an earlier document revision")]
    StaleDocument,
    #[error("document revision counter is exhausted")]
    RevisionExhausted,
    #[error("the original collection column was not found")]
    CollectionColumnMissing,
    #[error("the original collection parent was not found")]
    CollectionParentMissing,
    #[error("the selected field is not a stored collection")]
    NotCollection,
    #[error("file exceeds the 64 MiB read-session limit")]
    SnapshotLimit,
    #[error("the file changed while it was being opened")]
    SourceChanged,
    #[error("SQL exceeds the 64 KiB command limit")]
    SqlLimit,
    #[error("binary display exceeds the presentation limit")]
    BinaryLimit,
    #[error("unpublished expression bytes cannot be displayed")]
    UnpublishedString,
    #[error("text formatting failed")]
    Formatting,
    #[error(transparent)]
    Input(#[from] tablecraft_access::AccessInputError),
    #[error(transparent)]
    Native(#[from] tablecraft_ace::InspectError),
    #[error(transparent)]
    Io(#[from] tablecraft_io::IoError),
    #[error(transparent)]
    Parse(#[from] tablecraft_sql::ParseError),
    #[error(transparent)]
    Query(#[from] tablecraft_query::QueryError),
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    #[error(transparent)]
    Value(#[from] tablecraft_core::values::ValueError),
    #[error("allocation failed")]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Explicit host requests. Queued and consumed passwords are erased on drop.
pub enum Command {
    Open {
        path: PathBuf,
        password: String,
    },
    #[cfg(feature = "offline-edit")]
    BeginEdit {
        target: EditTarget,
    },
    #[cfg(feature = "offline-edit")]
    ReviewEdit {
        draft_id: u64,
        input: EditInput,
    },
    #[cfg(feature = "offline-edit")]
    BeginInsert {
        target: InsertTarget,
    },
    #[cfg(feature = "offline-edit")]
    ReviewInsert {
        draft_id: u64,
        inputs: Vec<InsertInput>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewDelete {
        target: DeleteTarget,
    },
    Page {
        table: String,
        offset: u32,
    },
    Describe {
        table: String,
    },
    InspectCollection {
        target: ContentsTarget,
    },
    Query {
        sql: String,
    },
    /// Complete bounded sorting through the query evaluator; results are read-only.
    SortTable {
        document_revision: u64,
        table: String,
        column: String,
        descending: bool,
    },
    InspectSavedQuery {
        target: SavedQueryTarget,
    },
    RunSavedQuery {
        target: SavedQueryTarget,
        parameters: Vec<ParameterInput>,
    },
}

impl Drop for Command {
    fn drop(&mut self) {
        if let Self::Open { password, .. } = self {
            password.zeroize();
        }
    }
}

pub struct Table {
    pub name: String,
    /// Local storage fact; independent of display text and row readability.
    pub local: bool,
    pub system: bool,
    pub storage: String,
}

pub enum Response {
    SavedQuery {
        target: SavedQueryTarget,
        parameters: Vec<ParameterSpec>,
    },
    #[cfg(feature = "offline-edit")]
    EditDraft(EditDraft),
    #[cfg(feature = "offline-edit")]
    EditReviewed {
        review: EditReview,
        capability: ReviewedEdit,
    },
    #[cfg(feature = "offline-edit")]
    InsertDraft(InsertDraft),
    #[cfg(feature = "offline-edit")]
    InsertReviewed {
        review: InsertReview,
        capability: ReviewedInsert,
    },
    #[cfg(feature = "offline-edit")]
    DeleteReviewed {
        review: DeleteReview,
        capability: ReviewedDelete,
    },
    /// A complete explicit read of one parent's stored collection. There are no
    /// executable attachment actions or borrowed native values in this response.
    Contents {
        target: ContentsTarget,
        kind: CollectionKind,
        assigned: bool,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
    },
    /// Inert declarations. Row totals are unverified; field reader nullability
    /// does not establish native Required/default/validation properties.
    Details {
        table: String,
        declared_rows: u32,
        fields: Vec<Field>,
    },
    Opened {
        document_revision: u64,
        /// Absolute path captured by the successful open operation.
        source_path: PathBuf,
        title: String,
        format: String,
        tables: Vec<Table>,
        queries: Vec<SavedQuery>,
    },
    Data {
        source_revision: Option<u64>,
        table: String,
        offset: u32,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
        has_more: bool,
    },
}

#[cfg(test)]
mod tests;
