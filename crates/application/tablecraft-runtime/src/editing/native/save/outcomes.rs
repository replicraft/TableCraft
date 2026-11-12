//! Allocation-free outcome facts derived from the retained actual resource owner.
mod current_field_addition;
#[cfg(feature = "ace16-create")]
mod database_options_field_history;
#[cfg(feature = "ace16-create")]
mod field_addition;
mod field_drop;
mod field_rename;
mod fresh_index_creation;
mod index_drop;
#[cfg(feature = "ace16-create")]
mod number_column;
mod query_expression_edits;
mod table_drop;

use super::RetainedOwner;
use crate::editing::save::{
    SaveCleanupDisposition, SaveCompletion, SaveDatabaseOptionsChange, SaveFailure,
    SaveFailureKind, SaveKnownOutcome, SaveOutcome, SavePhase, SaveQueryChange, SaveReceipt,
    SaveRetirementGuarantee, SaveSchemaChange, SaveSessionState,
};
use std::error::Error as StdError;
use tablecraft_access::editing::offline::{
    Cause, CompletionKind, DatabaseOptionsChange, Failure, KnownOutcome, OfflineWriteSession,
    OpenError, Phase, QueryChange, Receipt, SchemaChange, SessionState, VerifiedImage,
};
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddColumnError, AddColumnIntentError, AddTableError, AddTableIntentError, CreateDatabaseError,
    CreatedAddColumnError, DatabaseOptionsError, DatabaseOptionsIntentError,
    DefaultZeroAddColumnIntentError, FieldAdditionAfterOffHistoryIntentError,
    FieldAdditionAfterOffIntentError, FieldAdditionError, FieldAdditionIntentError,
    FieldAdditionSourceError, FirstTableError, FirstTableIntentError, IndexCreateIntentError,
    RepeatedDefaultZeroAddColumnIntentError,
};
use tablecraft_ace::{
    CurrentFieldAdditionError, CurrentFieldAdditionIntentError, DeclaredFieldAdditionIntentError,
    EditError, FreshIndexCreateIntentError, IndexDropIntentError, InspectError, NativeIntentError,
    NativeRecoveryQualificationError, QueryExpressionEditsError, QueryExpressionEditsIntentError,
    QueryExpressionError, QueryExpressionIntentError, QueryExpressionRecoveryError,
    RecoverableQueryExpressionError, TableRenameError, TableRenameIntentError,
    UnadornedFieldAdditionIntentError,
};
use tablecraft_core::limits::LimitError;
use tablecraft_io::{
    IoError,
    journal::{
        JournalError, RecoveryOutcome,
        filesystem::PairState,
        filesystem_contracts::{CleanupDisposition, RetirementGuarantee},
    },
};

const MAX_ERROR_SOURCE_DEPTH: usize = 32;

pub(super) fn retain_local(
    session: OfflineWriteSession,
    kind: SaveFailureKind,
    retained: &mut Option<RetainedOwner>,
) -> SaveOutcome {
    let mut failure = no_session(kind);
    failure.phase = Some(SavePhase::Planning);
    failure.state = Some(SaveSessionState::Ready);
    failure.last_completed = session.last_completed().map(receipt);
    failure.can_release = true;
    *retained = Some(RetainedOwner::Session(session));
    SaveOutcome::Failed(failure)
}

pub(super) fn finish(
    result: Result<(OfflineWriteSession, Receipt), Failure>,
    retained: &mut Option<RetainedOwner>,
) -> SaveOutcome {
    match result {
        Ok((session, completed)) => match session.release() {
            Ok(()) => SaveOutcome::Saved {
                receipt: receipt(completed),
            },
            Err(failure) => retain(failure, retained),
        },
        Err(failure) => retain(failure, retained),
    }
}

pub(super) fn failed_open(error: &OpenError) -> SaveOutcome {
    let kind = match error {
        OpenError::Native(error) => resource_failure(error),
        OpenError::Io(error) => resource_failure(error),
    };
    SaveOutcome::Failed(no_session(kind.unwrap_or(SaveFailureKind::Open)))
}

pub(super) fn no_session(kind: SaveFailureKind) -> SaveFailure {
    SaveFailure {
        kind,
        phase: None,
        state: None,
        current_outcome: None,
        last_completed: None,
        can_recover: false,
        can_release: false,
    }
}

pub(super) fn known(value: KnownOutcome) -> SaveKnownOutcome {
    match value {
        KnownOutcome::Native(RecoveryOutcome::Committed) => SaveKnownOutcome::Committed,
        KnownOutcome::Native(RecoveryOutcome::RolledBack) => SaveKnownOutcome::RolledBack,
        KnownOutcome::Verified(VerifiedImage::Original) => SaveKnownOutcome::OriginalVerified,
        KnownOutcome::Verified(VerifiedImage::Replacement) => SaveKnownOutcome::ReplacementVerified,
    }
}

pub(super) fn receipt(value: Receipt) -> SaveReceipt {
    SaveReceipt {
        completion: match value.kind() {
            CompletionKind::Applied => SaveCompletion::Applied,
            CompletionKind::Recovered => SaveCompletion::Recovered,
            CompletionKind::Confirmed => SaveCompletion::Confirmed,
        },
        known: value
            .outcome()
            .map(|value| known(KnownOutcome::Native(value)))
            .unwrap_or_else(|| {
                known(KnownOutcome::Verified(
                    value
                        .verified_image()
                        .expect("confirmation has a verified image"),
                ))
            }),
        matched_rows: value.matched_rows(),
        changed_rows: value.changed_rows(),
        schema_change: value.schema_change().map(|change| match change {
            SchemaChange::IndexCreated => SaveSchemaChange::IndexCreated,
            SchemaChange::IndexDropped => SaveSchemaChange::IndexDropped,
            SchemaChange::TableRenamed => SaveSchemaChange::TableRenamed,
            SchemaChange::TableDropped => SaveSchemaChange::TableDropped,
            SchemaChange::ColumnAdded => SaveSchemaChange::ColumnAdded,
            SchemaChange::ColumnRenamed => SaveSchemaChange::ColumnRenamed,
            SchemaChange::ColumnDropped => SaveSchemaChange::ColumnDropped,
            SchemaChange::FirstTableCreated { column_count } => {
                SaveSchemaChange::FirstTableCreated { column_count }
            }
            SchemaChange::TableAdded { column_count } => {
                SaveSchemaChange::TableAdded { column_count }
            }
        }),
        query_change: value.query_change().map(|change| match change {
            QueryChange::ExpressionSaved => SaveQueryChange::ExpressionSaved,
        }),
        database_options_change: value.database_options_change().map(|change| match change {
            DatabaseOptionsChange::NameAutoCorrectDisabled => {
                SaveDatabaseOptionsChange::NameAutoCorrectDisabled
            }
        }),
        retirement: value.retirement().map(|retirement| match retirement {
            RetirementGuarantee::CapturedParentBarrier => {
                SaveRetirementGuarantee::CapturedParentBarrier
            }
            RetirementGuarantee::NtfsWriteThroughRename => {
                SaveRetirementGuarantee::NtfsWriteThroughRename
            }
        }),
        cleanup: value.cleanup().map(|cleanup| match cleanup {
            CleanupDisposition::Removed => SaveCleanupDisposition::Removed,
            CleanupDisposition::RetainedArtifact => SaveCleanupDisposition::RetainedArtifact,
        }),
    }
}

pub(super) fn retain(failure: Failure, retained: &mut Option<RetainedOwner>) -> SaveOutcome {
    let phase = match failure.phase() {
        Phase::Planning => SavePhase::Planning,
        Phase::Intent => SavePhase::Intent,
        Phase::Setup => SavePhase::Setup,
        Phase::Prepare => SavePhase::Prepare,
        Phase::Commit => SavePhase::Commit,
        Phase::Recovery => SavePhase::Recovery,
        Phase::Confirmation => SavePhase::Confirmation,
        Phase::Refresh => SavePhase::Refresh,
        Phase::Finalization => SavePhase::Finalization,
        Phase::Release => SavePhase::Release,
    };
    let state = match failure.state() {
        SessionState::Ready => SaveSessionState::Ready,
        SessionState::RefreshRequired => SaveSessionState::RefreshRequired,
        SessionState::RetainedJournal => SaveSessionState::RetainedJournal,
        SessionState::IncompleteSetup => SaveSessionState::IncompleteSetup,
        SessionState::ReleaseUncertain => SaveSessionState::ReleaseUncertain,
        SessionState::Released => SaveSessionState::Released,
    };
    let kind = resource_failure(failure.cause()).unwrap_or_else(|| match failure.cause() {
        Cause::SourceChanged => SaveFailureKind::SourceChanged,
        Cause::Native(InspectError::Cancelled) => SaveFailureKind::Cancelled,
        Cause::Native(
            InspectError::Limit(_) | InspectError::Allocation | InspectError::StructureLimit,
        ) => SaveFailureKind::Resource,
        Cause::InvalidState => SaveFailureKind::InvalidState,
        _ => match phase {
            SavePhase::Planning => SaveFailureKind::Planning,
            SavePhase::Intent => SaveFailureKind::Intent,
            SavePhase::Setup => SaveFailureKind::Setup,
            SavePhase::Prepare => SaveFailureKind::Prepare,
            SavePhase::Commit => SaveFailureKind::Commit,
            SavePhase::Recovery | SavePhase::Confirmation => SaveFailureKind::Recovery,
            SavePhase::Finalization => SaveFailureKind::Finalization,
            SavePhase::Release => SaveFailureKind::Release,
            SavePhase::Refresh => SaveFailureKind::Planning,
        },
    });
    let current_outcome = failure.current_outcome().map(known);
    let last_completed = failure.last_completed().map(receipt);
    let report = SaveFailure {
        kind,
        phase: Some(phase),
        state: Some(state),
        current_outcome,
        last_completed,
        can_recover: state == SaveSessionState::RetainedJournal
            && current_outcome.is_none()
            && matches!(
                failure.pair_state(),
                Some(PairState::Ready | PairState::RecoveryRequired)
            ),
        can_release: !matches!(
            state,
            SaveSessionState::ReleaseUncertain | SaveSessionState::Released
        ),
    };
    *retained = Some(RetainedOwner::Failure(failure));
    SaveOutcome::Failed(report)
}

// Error inspection is bounded and allocation-free even after storage effects.
// The full original Failure remains owned; this only derives display-safe facts.
fn resource_failure(error: &(dyn StdError + 'static)) -> Option<SaveFailureKind> {
    let mut current = Some(error);
    for _ in 0..MAX_ERROR_SOURCE_DEPTH {
        let error = current?;
        if let Some(kind) = typed_resource_failure(error) {
            return Some(kind);
        }
        current = error.source();
    }
    None
}

// Transparent thiserror layers can forward past the wrapped error itself.
// Inspect the public typed variants before following the ordinary source chain.
fn typed_resource_failure(error: &(dyn StdError + 'static)) -> Option<SaveFailureKind> {
    if let Some(cause) = error.downcast_ref::<Cause>() {
        return match cause {
            Cause::Native(error) => inspection_failure(error),
            Cause::IndexDropIntent(error) => index_drop::intent(error),
            Cause::FreshIndexCreationIntent(error) => fresh_index_creation::intent(error),
            Cause::Intent(error) => intent_failure(error),
            Cause::Qualification(error) => qualification_failure(error),
            Cause::TableRenameIntent(error) => rename_intent_failure(error),
            Cause::TableDropIntent(error) => table_drop::intent(error),
            Cause::QueryExpressionEditsIntent(error) => query_expression_edits::intent(error),
            Cause::QueryExpressionIntent(error) => query_expression_intent_failure(error),
            Cause::QueryExpressionQualification(error) => query_expression_recovery_failure(error),
            Cause::Prepare(error) | Cause::Recovery(error) => journal_failure(error),
            Cause::Io(error) => io_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::FirstTablePlanning(error) => first_table_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::FirstTableIntent(error) => first_table_intent_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::AddTablePlanning(error) => add_table_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::AddTableIntent(error) => add_table_intent_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::IndexCreationIntent(error) => index_creation_intent_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::DefaultZeroAddColumnIntent(error) => number_column::intent(error),
            #[cfg(feature = "ace16-create")]
            Cause::FieldAdditionIntent(error) => field_addition::intent(error),
            Cause::CurrentFieldAdditionIntent(error) => current_field_addition::intent(error),
            Cause::DeclaredFieldAdditionIntent(error) => current_field_addition::declared(error),
            Cause::UnadornedFieldAdditionIntent(error) => current_field_addition::unadorned(error),
            Cause::RetainedTypedFieldAdditionIntent(error) => {
                current_field_addition::retained_typed(error)
            }
            Cause::FieldRenameIntent(error) => field_rename::intent(error),
            Cause::FieldDropIntent(error) => field_drop::intent(error),
            #[cfg(feature = "ace16-create")]
            Cause::FieldAdditionAfterOffIntent(error) => field_addition_after_off::intent(error),
            #[cfg(feature = "ace16-create")]
            Cause::FieldAdditionAfterOffHistoryIntent(error) => {
                field_addition_after_off_history::intent(error)
            }
            #[cfg(feature = "ace16-create")]
            Cause::RepeatedDefaultZeroAddColumnIntent(error) => {
                number_column::repeated_intent(error)
            }
            #[cfg(feature = "ace16-create")]
            Cause::AddColumnIntent(error) => add_column_intent_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::DatabaseOptionsIntent(error) => database_options_intent_failure(error),
            #[cfg(feature = "ace16-create")]
            Cause::FieldHistoryDatabaseOptionsIntent(error) => {
                database_options_field_history::intent(error)
            }
            _ => None,
        };
    }
    if let Some(error) = error.downcast_ref::<NativeRecoveryQualificationError>() {
        return qualification_failure(error);
    }
    if let Some(error) = error.downcast_ref::<tablecraft_ace::TableDropError>() {
        return table_drop::candidate(error);
    }
    if let Some(error) = error.downcast_ref::<tablecraft_ace::TableDropIntentError>() {
        return table_drop::intent(error);
    }
    if let Some(error) = error.downcast_ref::<TableRenameError>() {
        return rename_failure(error);
    }
    if let Some(error) = error.downcast_ref::<TableRenameIntentError>() {
        return rename_intent_failure(error);
    }
    if let Some(error) = error.downcast_ref::<QueryExpressionEditsError>() {
        return query_expression_edits::expression(error);
    }
    if let Some(error) = error.downcast_ref::<QueryExpressionEditsIntentError>() {
        return query_expression_edits::intent(error);
    }
    if let Some(error) = error.downcast_ref::<QueryExpressionError>() {
        return query_expression_failure(error);
    }
    if let Some(error) = error.downcast_ref::<QueryExpressionIntentError>() {
        return query_expression_intent_failure(error);
    }
    if let Some(error) = error.downcast_ref::<QueryExpressionRecoveryError>() {
        return query_expression_recovery_failure(error);
    }
    if let Some(error) = error.downcast_ref::<RecoverableQueryExpressionError>() {
        return query_expression_planning_failure(error);
    }
    if let Some(error) = error.downcast_ref::<NativeIntentError>() {
        return intent_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FirstTableError>() {
        return first_table_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FirstTableIntentError>() {
        return first_table_intent_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<AddTableError>() {
        return add_table_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<AddTableIntentError>() {
        return add_table_intent_failure(error);
    }
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldDropIntentError>() {
        return field_drop::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldDropError>() {
        return field_drop::candidate(error);
    }
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldRenameIntentError>() {
        return field_rename::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldRenameError>() {
        return field_rename::candidate(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FieldAdditionIntentError>() {
        return field_addition::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FieldAdditionAfterOffIntentError>() {
        return field_addition_after_off::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FieldAdditionAfterOffHistoryIntentError>() {
        return field_addition_after_off_history::intent(error);
    }
    if let Some(error) =
        error.downcast_ref::<tablecraft_ace::RetainedTypedFieldAdditionIntentError>()
    {
        return current_field_addition::retained_typed(error);
    }
    if let Some(error) = error.downcast_ref::<DeclaredFieldAdditionIntentError>() {
        return current_field_addition::declared(error);
    }
    if let Some(error) = error.downcast_ref::<UnadornedFieldAdditionIntentError>() {
        return current_field_addition::unadorned(error);
    }
    if let Some(error) = error.downcast_ref::<CurrentFieldAdditionIntentError>() {
        return current_field_addition::intent(error);
    }
    if let Some(error) = error.downcast_ref::<CurrentFieldAdditionError>() {
        return current_field_addition::candidate(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FieldAdditionSourceError>() {
        return field_addition_source::failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<FieldAdditionError>() {
        return field_addition::candidate(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<RepeatedDefaultZeroAddColumnIntentError>() {
        return number_column::repeated_intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<DefaultZeroAddColumnIntentError>() {
        return number_column::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::CreatedDefaultZeroAddColumnError>() {
        return number_column::candidate(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<AddColumnIntentError>() {
        return add_column_intent_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<AddColumnError>() {
        return add_column_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldHistoryOptionsIntentError>() {
        return database_options_field_history::intent(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::FieldHistoryDatabaseOptionsError>() {
        return database_options_field_history::candidate(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<tablecraft_ace::DatabaseOptionsSourceError>() {
        return database_options_field_history::source(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<DatabaseOptionsIntentError>() {
        return database_options_intent_failure(error);
    }
    #[cfg(feature = "ace16-create")]
    if let Some(error) = error.downcast_ref::<DatabaseOptionsError>() {
        return database_options_failure(error);
    }
    if let Some(error) = error.downcast_ref::<JournalError>() {
        return journal_failure(error);
    }
    if let Some(error) = error.downcast_ref::<InspectError>() {
        return inspection_failure(error);
    }
    if let Some(error) = error.downcast_ref::<FreshIndexCreateIntentError>() {
        return fresh_index_creation::intent(error);
    }
    if let Some(error) = error.downcast_ref::<IndexDropIntentError>() {
        return index_drop::intent(error);
    }
    if let Some(error) = error.downcast_ref::<IoError>() {
        return io_failure(error);
    }
    error
        .downcast_ref::<LimitError>()
        .map(|_| SaveFailureKind::Resource)
}

#[cfg(feature = "ace16-create")]
fn index_creation_intent_failure(error: &IndexCreateIntentError) -> Option<SaveFailureKind> {
    match error {
        IndexCreateIntentError::Inspect(error) => inspection_failure(error),
        IndexCreateIntentError::Edit(EditError::Inspect(error)) => inspection_failure(error),
        IndexCreateIntentError::Proof(error) => qualification_failure(error),
        IndexCreateIntentError::Binding(error) => intent_failure(error),
        _ => None,
    }
}

fn intent_failure(error: &NativeIntentError) -> Option<SaveFailureKind> {
    match error {
        NativeIntentError::Inspect(error)
        | NativeIntentError::Request(EditError::Inspect(error)) => inspection_failure(error),
        _ => None,
    }
}

fn qualification_failure(error: &NativeRecoveryQualificationError) -> Option<SaveFailureKind> {
    match error {
        NativeRecoveryQualificationError::Rename(error) => rename_failure(error),
        NativeRecoveryQualificationError::TableRenameIntent(error) => rename_intent_failure(error),
        NativeRecoveryQualificationError::Inspect(error)
        | NativeRecoveryQualificationError::Edit(EditError::Inspect(error)) => {
            inspection_failure(error)
        }
        NativeRecoveryQualificationError::Intent(error) => intent_failure(error),
        #[cfg(feature = "ace16-create")]
        NativeRecoveryQualificationError::FirstTable(error) => first_table_failure(error),
        #[cfg(feature = "ace16-create")]
        NativeRecoveryQualificationError::FirstTableIntent(error) => {
            first_table_intent_failure(error)
        }
        #[cfg(feature = "ace16-create")]
        NativeRecoveryQualificationError::AddTable(error) => add_table_failure(error),
        #[cfg(feature = "ace16-create")]
        NativeRecoveryQualificationError::AddTableIntent(error) => add_table_intent_failure(error),
        _ => None,
    }
}

fn query_expression_failure(error: &QueryExpressionError) -> Option<SaveFailureKind> {
    match error {
        QueryExpressionError::Inspect(error) => inspection_failure(error),
        _ => None,
    }
}

fn query_expression_intent_failure(error: &QueryExpressionIntentError) -> Option<SaveFailureKind> {
    match error {
        QueryExpressionIntentError::Inspect(error) => inspection_failure(error),
        QueryExpressionIntentError::Binding(error) => intent_failure(error),
        _ => None,
    }
}

fn query_expression_recovery_failure(
    error: &QueryExpressionRecoveryError,
) -> Option<SaveFailureKind> {
    match error {
        QueryExpressionRecoveryError::Inspect(error) => inspection_failure(error),
        QueryExpressionRecoveryError::Intent(error) => query_expression_intent_failure(error),
        QueryExpressionRecoveryError::Expression(error) => query_expression_failure(error),
        QueryExpressionRecoveryError::Proof(error) => qualification_failure(error),
    }
}

fn query_expression_planning_failure(
    error: &RecoverableQueryExpressionError,
) -> Option<SaveFailureKind> {
    match error {
        RecoverableQueryExpressionError::Inspect(error) => inspection_failure(error),
        RecoverableQueryExpressionError::Intent(error) => query_expression_intent_failure(error),
        RecoverableQueryExpressionError::Expression(error) => query_expression_failure(error),
    }
}

fn rename_failure(error: &TableRenameError) -> Option<SaveFailureKind> {
    match error {
        TableRenameError::Inspect(error) => inspection_failure(error),
        _ => None,
    }
}

fn rename_intent_failure(error: &TableRenameIntentError) -> Option<SaveFailureKind> {
    match error {
        TableRenameIntentError::Inspect(error) => inspection_failure(error),
        TableRenameIntentError::Binding(error) => intent_failure(error),
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn first_table_failure(error: &FirstTableError) -> Option<SaveFailureKind> {
    match error {
        FirstTableError::Inspect(error)
        | FirstTableError::Create(CreateDatabaseError::Inspect(error)) => inspection_failure(error),
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn first_table_intent_failure(error: &FirstTableIntentError) -> Option<SaveFailureKind> {
    match error {
        FirstTableIntentError::Inspect(error)
        | FirstTableIntentError::Create(CreateDatabaseError::Inspect(error)) => {
            inspection_failure(error)
        }
        FirstTableIntentError::Binding(error) => intent_failure(error),
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn add_table_failure(error: &AddTableError) -> Option<SaveFailureKind> {
    match error {
        AddTableError::Inspect(error)
        | AddTableError::Create(CreateDatabaseError::Inspect(error)) => inspection_failure(error),
        AddTableError::Create(_)
        | AddTableError::Source
        | AddTableError::DuplicateName
        | AddTableError::Capacity
        | AddTableError::Preservation => None,
    }
}

#[cfg(feature = "ace16-create")]
fn add_table_intent_failure(error: &AddTableIntentError) -> Option<SaveFailureKind> {
    match error {
        AddTableIntentError::Inspect(error) => inspection_failure(error),
        AddTableIntentError::Binding(error) => intent_failure(error),
        AddTableIntentError::Declaration(error) => first_table_intent_failure(error),
        AddTableIntentError::InvalidRecord
        | AddTableIntentError::UnsupportedVersion
        | AddTableIntentError::UnsupportedProfile
        | AddTableIntentError::BeforeMismatch => None,
    }
}

#[cfg(feature = "ace16-create")]
fn add_column_failure(error: &AddColumnError) -> Option<SaveFailureKind> {
    match error {
        AddColumnError::Inspect(error)
        | AddColumnError::Declaration(CreateDatabaseError::Inspect(error)) => {
            inspection_failure(error)
        }
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn add_column_intent_failure(error: &AddColumnIntentError) -> Option<SaveFailureKind> {
    match error {
        AddColumnIntentError::Inspect(error) => inspection_failure(error),
        AddColumnIntentError::Binding(error) => intent_failure(error),
        AddColumnIntentError::Column(error) => add_column_failure(error),
        AddColumnIntentError::CreatedColumn(
            CreatedAddColumnError::Inspect(error)
            | CreatedAddColumnError::Declaration(CreateDatabaseError::Inspect(error)),
        ) => inspection_failure(error),
        AddColumnIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn database_options_failure(error: &DatabaseOptionsError) -> Option<SaveFailureKind> {
    match error {
        DatabaseOptionsError::Inspect(error)
        | DatabaseOptionsError::Create(CreateDatabaseError::Inspect(error)) => {
            inspection_failure(error)
        }
        _ => None,
    }
}

#[cfg(feature = "ace16-create")]
fn database_options_intent_failure(error: &DatabaseOptionsIntentError) -> Option<SaveFailureKind> {
    match error {
        DatabaseOptionsIntentError::Inspect(error) => inspection_failure(error),
        DatabaseOptionsIntentError::Binding(error) => intent_failure(error),
        DatabaseOptionsIntentError::Options(error) => database_options_failure(error),
        DatabaseOptionsIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

fn journal_failure(error: &JournalError) -> Option<SaveFailureKind> {
    match error {
        JournalError::Io(error) => io_failure(error),
        JournalError::Allocation | JournalError::Limit(_) => Some(SaveFailureKind::Resource),
        _ => None,
    }
}

fn io_failure(error: &IoError) -> Option<SaveFailureKind> {
    match error {
        IoError::Cancelled => Some(SaveFailureKind::Cancelled),
        IoError::Limit(_) => Some(SaveFailureKind::Resource),
        _ => None,
    }
}

fn inspection_failure(error: &InspectError) -> Option<SaveFailureKind> {
    match error {
        InspectError::Cancelled => Some(SaveFailureKind::Cancelled),
        InspectError::Limit(_) | InspectError::Allocation | InspectError::StructureLimit => {
            Some(SaveFailureKind::Resource)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "ace16-create")]
mod field_addition_after_off;

#[cfg(feature = "ace16-create")]
mod field_addition_source;

#[cfg(feature = "ace16-create")]
mod field_addition_after_off_history;
