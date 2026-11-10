//! Typed application task failures without credentials or row payloads.
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use tablecraft_ace::{
    AddColumnIntentError, AddTableError, CurrentFieldAdditionIntentError,
    DefaultZeroAddColumnIntentError, FieldAdditionAfterOffHistoryIntentError,
    FieldAdditionAfterOffIntentError, FieldAdditionIntentError, FieldAdditionSourceError,
    FirstTableError, RepeatedDefaultZeroAddColumnIntentError,
};

#[cfg(feature = "offline-edit")]
use crate::{EditError, QuerySqlEditError, ReplaceError};
use crate::{FilterError, FindError, TotalsError};
use tablecraft_builders::query::QueryDesignError;

/// A typed task failure. Display text contains no credentials or row payloads.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("read query {pool:?} pool: {source}")]
    ReadQueryLimit {
        pool: crate::QueryRunPool,
        #[source]
        source: tablecraft_core::limits::LimitError,
    },
    /// The read actor ended; no cached document or queued command is reused.
    #[error("the background read worker stopped unexpectedly")]
    ReadWorkerStopped,
    /// No fresh read identity remains; recreate the worker and reopen explicitly.
    #[error(transparent)]
    ReadRequestExhausted(#[from] crate::ReadRequestExhausted),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    DatabaseOptions(#[from] tablecraft_ace::DatabaseOptionsIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldHistoryDatabaseOptions(#[from] tablecraft_ace::FieldHistoryOptionsIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    DatabaseOptionsSource(#[from] tablecraft_ace::DatabaseOptionsSourceError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("database option contents changed before reopening")]
    DatabaseOptionsSourceChanged,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    QueryExpression(#[from] tablecraft_ace::RecoverableQueryExpressionError),
    #[cfg(feature = "offline-edit")]
    #[error("saved query contents changed before reopening")]
    QueryExpressionSourceChanged,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    QuerySqlEdit(#[from] QuerySqlEditError),
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    TableRename(#[from] tablecraft_ace::RecoverableTableRenameError),
    #[cfg(feature = "offline-edit")]
    #[error("renamed table contents changed before reopening")]
    RenamedTableSourceChanged,
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    TableDropSource(#[from] tablecraft_ace::TableDropError),
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    TableDropIntent(#[from] tablecraft_ace::TableDropIntentError),
    #[cfg(feature = "offline-edit")]
    #[error("table DROP target no longer matches the viewed document")]
    StaleTableDropTarget,
    #[cfg(feature = "offline-edit")]
    #[error("deleted table source changed before reopening")]
    TableDroppedSourceChanged,
    #[cfg(feature = "offline-edit")]
    #[error("original insertion source changed before reopening")]
    InsertOriginalSourceChanged,
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    AddColumn(#[from] AddColumnIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    DefaultZeroAddColumn(#[from] DefaultZeroAddColumnIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    RepeatedDefaultZeroAddColumn(#[from] RepeatedDefaultZeroAddColumnIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldAddition(#[from] FieldAdditionIntentError),
    /// Exact ADD-after-OFF original request or complete native reconstruction refused.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldAdditionAfterOff(#[from] FieldAdditionAfterOffIntentError),
    /// Original repeated post-OFF field request or complete native replay refused.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldAdditionAfterOffHistory(#[from] FieldAdditionAfterOffHistoryIntentError),
    /// Current metadata Number declaration or fresh reconstruction refused.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    CurrentFieldAddition(#[from] CurrentFieldAdditionIntentError),
    /// Selected native field source or readonly capacity qualification failed.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldAdditionSource(#[from] FieldAdditionSourceError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldRenameSource(#[from] tablecraft_ace::FieldRenameError),
    /// Source-only DROP inspection refused outside an explicit unsupported leaf.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldDropSource(#[from] tablecraft_ace::FieldDropError),
    /// Exact original native field name/number does not match the current source.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("saved-field DROP target is no longer current")]
    StaleFieldDropTarget,
    /// Original request, native source or complete DROP reconstruction failed.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldDropIntent(#[from] tablecraft_ace::FieldDropIntentError),
    /// Complete checked DROP replacement differs from the accepted read witness.
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("saved-field DROP image changed before reopening")]
    FieldDroppedSourceChanged,
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("native field identity no longer matches the selected target")]
    StaleFieldRenameTarget,
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error(transparent)]
    FieldRenameIntent(#[from] tablecraft_ace::FieldRenameIntentError),
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("saved field rename contents changed before reopening")]
    FieldRenamedSourceChanged,
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    #[error("saved column contents changed before reopening")]
    AddedColumnSourceChanged,
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
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    QueryUpdate(#[from] tablecraft_access::editing::sql::RecoverableError),
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    QueryDelete(#[from] tablecraft_access::editing::sql::RecoverableDeleteError),
    #[cfg(feature = "offline-edit")]
    #[error(transparent)]
    QueryInsert(#[from] tablecraft_access::editing::sql::RecoverableInsertError),
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
    #[error("query inspection belongs to another document snapshot")]
    StaleQuery,
    #[error("query parameter inputs do not match their declarations")]
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
    #[error("native image exceeds this operation's admission or address space")]
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
