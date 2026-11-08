//! Shared application sessions and cancellable native read tasks for hosts.
mod cell;
mod column;
mod contents;
mod controls;
#[cfg(feature = "ace16-create")]
mod creation;
mod display;
mod document;
#[cfg(feature = "offline-edit")]
mod editing;
mod filtering;
mod find;
mod native_properties;
mod query;
mod query_design;
mod rows;
mod saved;
mod scalar_input;
mod schema;
mod source;
mod totals;
mod worker;

pub use cell::{Cell, CellKind, DisplayControl, HorizontalAlignment};
pub use column::Column;
pub use contents::{CollectionKind, ContentsTarget};
#[cfg(feature = "ace16-create")]
pub use creation::{
    CreationFailure, CreationOperation, CreationOutcome, CreationPhase, CreationPublication,
    CreationReceipt, CreationReport, CreationRequest, CreationStartError, CreationStartFailure,
    CreationWorker,
};
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
pub use editing::{CreatedTableReopen, ReviewedTableCreation};
#[cfg(feature = "offline-edit")]
pub use editing::{
    DeleteReview, DeleteTarget, EditDraft, EditError, EditInput, EditReview, EditTarget,
    InsertDraft, InsertField, InsertInput, InsertRetryEvidence, InsertRetryWitness, InsertReview,
    InsertTarget, ReplaceError, ReviewedDelete, ReviewedEdit, ReviewedInsert,
    SaveCleanupDisposition, SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome,
    SaveOperation, SaveOutcome, SavePhase, SaveReceipt, SaveReport, SaveRetirementGuarantee,
    SaveSchemaChange, SaveSessionState, SaveStartError, SaveStartFailure, SaveWorker, journal_path,
};
pub use filtering::{
    FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
    FilterComparison, FilterDefinition, FilterError, FilterHandle, FilterSelectionTarget,
    FilterTarget, FilteredData, MAX_FIELD_FILTER_INPUT_BYTES, MAX_FILTER_FIELDS, SortSpec,
    SortTarget,
};
pub use find::{
    FindDirection, FindError, FindMatch, FindPosition, FindRequest, FindScope, FindSpan,
    FindTarget, MAX_FIND_PATTERN_BYTES,
};
pub use query_design::QueryDesignTarget;
pub use saved::{
    MAX_SAVED_QUERY_INPUT_BYTES, MAX_SAVED_QUERY_PARAMETERS, ParameterInput, ParameterSpec,
    SavedQuery, SavedQueryTarget,
};
pub use scalar_input::InputHint;
pub use schema::{Field, FieldProperty, FieldPropertyValue};
pub use totals::{
    MAX_TOTAL_FIELDS, TotalAggregate, TotalSpec, TotalsData, TotalsDescription, TotalsError,
    TotalsField, TotalsTarget,
};
pub use worker::Worker;

use std::path::PathBuf;
#[cfg(any(feature = "ace16-create", feature = "offline-edit"))]
use std::sync::Arc;
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use tablecraft_ace::{AddTableError, FirstTableError};
use tablecraft_builders::query::{
    BuiltQueryDesign, DesignTableMetadata, QueryDesignDraft, QueryDesignError,
};
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use tablecraft_builders::table::TableDraftSnapshot;
use zeroize::Zeroize;

/// A typed task failure. Display text contains no credentials or row payloads.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "offline-edit")]
    #[error("original insertion source changed before reopening")]
    InsertOriginalSourceChanged,
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FirstTable(#[from] FirstTableError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    AddTable(#[from] AddTableError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("saved table contents changed before reopening")]
    SavedTableSourceChanged,
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Creation(#[from] tablecraft_ace::CreateDatabaseError),
    #[cfg(feature = "ace16-create")]
    #[error("creation report does not acknowledge a published database")]
    NoCreatedReceipt,
    #[cfg(feature = "ace16-create")]
    #[error("created database contents changed before opening")]
    CreatedSourceChanged,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    Replace(#[from] ReplaceError),
    #[error(transparent)]
    QueryDesign(#[from] QueryDesignError),
    #[error(transparent)]
    Totals(#[from] TotalsError),
    #[cfg(feature = "offline-edit")]
    #[error("a reserved journal entry requires explicit recovery or diagnosis before opening")]
    RetainedJournal,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error(transparent)]
    Filter(#[from] FilterError),
    #[error(transparent)]
    Find(#[from] FindError),
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
    /// Read-only guarded reopening; hosts separately decide whether retry is safe.
    #[cfg(feature = "offline-edit")]
    OpenInsertOriginal {
        witness: Arc<InsertRetryWitness>,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewTableCreation {
        document_revision: u64,
        snapshot: TableDraftSnapshot,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenTableCreated {
        witness: Arc<CreatedTableReopen>,
    },
    /// Reopen only an acknowledged creation whose fresh snapshot still matches
    /// the canonical blank image. This does not attest physical file identity.
    #[cfg(feature = "ace16-create")]
    OpenCreated {
        report: Arc<CreationReport>,
    },
    #[cfg(feature = "offline-edit")]
    ReplaceFindMatch {
        request: FindRequest,
        found: FindPosition,
        replacement: String,
        filter: Option<(FilterHandle, bool)>,
    },
    DescribeQueryDesign {
        target: QueryDesignTarget,
        table: String,
    },
    BuildQueryDesign {
        target: QueryDesignTarget,
        draft: QueryDesignDraft,
    },
    ImportQueryDesign {
        target: QueryDesignTarget,
        sql: String,
    },
    DescribeTotals {
        target: TotalsTarget,
    },
    Totals {
        target: TotalsTarget,
        specs: Vec<TotalSpec>,
        base: Option<FilterHandle>,
    },
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
    SortView {
        target: SortTarget,
        sort: Option<SortSpec>,
        base: Option<FilterHandle>,
        enabled: bool,
    },
    ClearFilter {
        handle: FilterHandle,
    },
    ReapplyFilter {
        definition: FilterDefinition,
        document_revision: u64,
        enabled: bool,
        offset: u32,
    },
    DescribeFieldFilter {
        target: FieldFilterTarget,
    },
    ApplyFieldFilter {
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: Vec<String>,
        base: Option<FilterHandle>,
    },
    FilterSelection {
        target: FilterSelectionTarget,
        comparison: FilterComparison,
        base: Option<FilterHandle>,
    },
    FilterPage {
        handle: FilterHandle,
        offset: u32,
        enabled: bool,
    },
    FindFiltered {
        handle: FilterHandle,
        request: FindRequest,
        enabled: bool,
    },
    FindNext {
        request: FindRequest,
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
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    TableCreationReviewed {
        capability: ReviewedTableCreation,
    },
    QueryDesignTable {
        target: QueryDesignTarget,
        metadata: DesignTableMetadata,
    },
    QueryDesignBuilt {
        target: QueryDesignTarget,
        built: BuiltQueryDesign,
        metadata: Vec<DesignTableMetadata>,
    },
    TotalsDescription(TotalsDescription),
    Totals(TotalsData),
    FieldFilterDescription(FieldFilterDescription),
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
    FilteredData(FilteredData),
    Found {
        request: FindRequest,
        position: Option<FindPosition>,
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
        /// Exact captured native source admission, tied to document_revision.
        /// This is not a draft or permission to bypass fresh review/save checks.
        table_creation_available: bool,
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
