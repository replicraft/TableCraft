//! Shared database workspace presentation; hosts own files, tasks and services.
mod application_icon;
mod cells;
mod columns;
mod datasheet;
mod fonts;
mod formatting;
mod keyboard;
pub mod localization;
mod text_input;
mod theme;
mod workspace;

pub use application_icon::{ApplicationIconError, application_icon};
pub use cells::{Cell, CellKind, DisplayControl, HorizontalAlignment};
pub use columns::Column;
pub use fluent_bundle::FluentArgs;
pub use fonts::{FontFamilies, FontInventoryError};
pub use localization::{Locale, Localizer};
pub use theme::{Theme, configure};
pub use workspace::preview;
pub use workspace::{
    Action, BooleanToggle, CollectionKind, ContentsTarget, CreationOutcome, CreationRequest, Data,
    Database, DeleteReview, DeleteTarget, EditDraft, EditInput, EditReview, EditSaveOutcome,
    EditSeed, EditTarget, Field, FieldAdditionChoices, FieldAdditionInspectionError,
    FieldAdditionRequest, FieldAdditionSaveCommand, FieldAdditionSaveIdentity,
    FieldAdditionSavedWarning, FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode,
    FieldFilterTarget, FieldProperty, FieldPropertyValue, FilterComparison, FilterPublication,
    FilterRestore, FilterSelectionTarget, FilterTarget, FindDirection, FindMatch, FindPosition,
    FindRequest, FindScope, FindSpan, FindTarget, InitialRecord, InputHint, InsertDraft,
    InsertField, InsertInput, InsertRetryPermission, InsertReview, InsertTarget, ObjectKind,
    ObjectOpen, ObjectTab, ObjectTarget, ParameterInput, ParameterSpec, PreparedFieldAdditionSave,
    QueryActionEffect, QueryActionKind, QueryActionReview, QueryCollectionBinding,
    QueryContentsBindings, QueryContentsTarget, QueryCreationRequest, QueryCreationSaveCommand,
    QueryCreationSaveIdentity, QueryCreationSaveOperation, QueryDesignCompletion,
    QueryDesignRequest, QueryDesignTarget, QuerySchemaEffect, QuerySchemaPublication, RecordTarget,
    RecoveryOutcome, ReplaceFindRequest, RowOrdinalError, RowOrdinals, SaveCleanupNotice,
    SavedQuery, SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult, SavedQuerySqlEligibility,
    SavedQuerySqlError, SavedQuerySqlRequest, SavedQuerySqlSaveCommand, SavedQuerySqlSaveIdentity,
    SavedQuerySqlSaveOperation, SavedQueryTarget, Schema, SortSpec, SortTarget, State, Table,
    TableDraftSaveCompletion, TableDropRequest, TableDropSaveCommand, TableDropSaveIdentity,
    TableRenameRequest, TableSaveRequest, TotalAggregate, TotalSpec, TotalsData, TotalsDescription,
    TotalsField, TotalsTarget, WorkspaceLayout,
};
pub use workspace::{
    DatabaseOptionsCloseRequest, DatabaseOptionsSaveCommand, DatabaseOptionsSaveIdentity,
    NameAutoCorrectState, PreparedDatabaseOptionsSave,
};
pub use workspace::{FieldAdditionCloseDecision, FieldAdditionCloseResult};
pub use workspace::{
    FieldDropCloseDecision, FieldDropCloseResult, FieldDropConfirmationDecision,
    FieldDropConfirmationResult, FieldDropRequest, FieldDropSaveCommand, FieldDropSaveIdentity,
    FieldDropTarget, FieldDropWarning, PreparedFieldDropInspection, PreparedFieldDropResolution,
    PreparedFieldDropSave,
};
pub use workspace::{
    FieldRenameCloseDecision, FieldRenameCloseResult, PreparedCleanFieldRenameClose,
};
pub use workspace::{
    FieldRenameRequest, FieldRenameSaveCommand, FieldRenameSaveIdentity, FieldRenameTarget,
    FieldRenameWarning, PreparedFieldRenameInspection, PreparedFieldRenameResolution,
    PreparedFieldRenameSave, PreparedUnchangedFieldRename,
};
pub use workspace::{QueryPoolLimits, QueryRunLimits};
pub use workspace::{UpdateFailure, UpdateManualReason, UpdateStatus};

#[cfg(test)]
mod tests;
