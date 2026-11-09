//! Shared application sessions and cancellable native read tasks for hosts.
mod cell;
mod column;
mod command;
mod contents;
mod controls;
#[cfg(feature = "ace16-create")]
mod creation;
mod display;
mod document;
#[cfg(feature = "offline-edit")]
mod editing;
mod error;
mod filtering;
mod find;
mod native_properties;
mod query;
mod query_design;
mod response;
mod rows;
mod saved;
mod scalar_input;
mod schema;
mod source;
mod table;
mod task_signal;
mod thread_start;
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
pub use editing::{
    AddColumnReview, AddedColumnReopen, CreatedTableReopen, DatabaseOptionsInspection,
    DatabaseOptionsOutcomeError, DatabaseOptionsReopen, DatabaseOptionsReopenFacts,
    DatabaseOptionsReview, FieldAdditionAvailability, FieldAdditionOutcomeError,
    FieldAdditionReopenFacts, FieldAdditionReview, FieldRenameOutcomeError, FieldRenameReopenFacts,
    FieldRenameReview, FieldRenameSourceInspection, FieldRenamedReopen, NameAutoCorrectState,
    NumberColumnAvailability, ReviewedAddColumn, ReviewedDatabaseOptions, ReviewedFieldAddition,
    ReviewedFieldRename, ReviewedNumberColumn, ReviewedTableCreation,
};
#[cfg(feature = "offline-edit")]
pub use editing::{
    DeleteReview, DeleteTarget, EditDraft, EditError, EditInput, EditReview, EditTarget,
    InsertDraft, InsertField, InsertInput, InsertRetryEvidence, InsertRetryWitness, InsertReview,
    InsertTarget, QueryDeleteReview, QueryExpressionOutcomeError, QueryExpressionReopen,
    QueryExpressionReopenFacts, QueryExpressionReview, QueryInsertReview, QueryUpdateReview,
    ReplaceError, ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryDelete,
    ReviewedQueryExpression, ReviewedQueryInsert, ReviewedQueryUpdate, ReviewedTableRename,
    SaveCleanupDisposition, SaveCompletion, SaveDatabaseOptionsChange, SaveFailure,
    SaveFailureKind, SaveKnownOutcome, SaveOperation, SaveOutcome, SavePhase, SaveQueryChange,
    SaveReceipt, SaveReport, SaveRetirementGuarantee, SaveSchemaChange, SaveSessionState,
    SaveStartError, SaveStartFailure, SaveWorker, TableRenameReopen, TableRenameReview,
    journal_path,
};
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
pub use editing::{
    FieldDropOutcomeError, FieldDropReopenFacts, FieldDropReview, FieldDropSourceInspection,
    FieldDroppedReopen, ReviewedFieldDrop,
};
#[cfg(feature = "offline-edit")]
pub use editing::{
    QuerySqlDraft, QuerySqlEditError, QuerySqlEligibility, QuerySqlReview, ReviewedQuerySql,
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
pub use query::inspection::{InspectedQuery, QueryOperation};
pub use query::parameters::{
    MAX_QUERY_INPUT_BYTES, MAX_QUERY_PARAMETERS, ParameterInput, ParameterSpec,
};
pub use query_design::QueryDesignTarget;
pub use saved::{SavedQuery, SavedQueryTarget};
pub use scalar_input::InputHint;
pub use schema::{Field, FieldProperty, FieldPropertyValue};
pub use totals::{
    MAX_TOTAL_FIELDS, TotalAggregate, TotalSpec, TotalsData, TotalsDescription, TotalsError,
    TotalsField, TotalsTarget,
};
pub use worker::Worker;

pub use command::Command;
pub use error::Error;
pub use response::Response;
pub use table::Table;

#[cfg(test)]
mod tests;
