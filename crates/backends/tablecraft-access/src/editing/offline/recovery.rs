use super::plan::MutationIntent;
use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt,
    SessionState, VerifiedImage,
};
use std::path::Path;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddTableSourceProfile, NativeAddColumnIntent, NativeAddTableIntent,
    NativeCurrentFieldAdditionIntent, NativeDatabaseOptionsIntent,
    NativeDefaultZeroAddColumnIntent, NativeFieldAdditionAfterOffHistoryIntent,
    NativeFieldAdditionAfterOffIntent, NativeFieldAdditionIntent, NativeFieldDropIntent,
    NativeFieldHistoryDatabaseOptionsIntent, NativeFieldRenameIntent, NativeFirstTableIntent,
    NativePopulatedAddTableIntent, NativeRepeatedDefaultZeroAddColumnIntent,
    qualify_native_add_column_recovery, qualify_native_add_table_recovery,
    qualify_native_current_field_addition_recovery,
    qualify_native_default_zero_add_column_recovery,
    qualify_native_field_addition_after_off_history_recovery,
    qualify_native_field_addition_after_off_recovery, qualify_native_field_addition_recovery,
    qualify_native_field_drop_recovery, qualify_native_field_rename_recovery,
    qualify_native_first_table_recovery, qualify_native_repeated_default_zero_add_column_recovery,
};
use tablecraft_ace::{
    NativeClockInsertIntent, NativeCompleteInsertIntent, NativeDeleteIntent, NativeEofInsertIntent,
    NativeFittedEditIntent, NativeInsertIntent, NativeOmittedInsertIntent,
    NativeQueryExpressionEditsIntent, NativeQueryExpressionIntent, NativeTableDropIntent,
    NativeTableRenameIntent, NativeUpdateIntent, NativeUpdateProfile,
    qualify_native_allocation_recovery, qualify_native_clock_insert_recovery,
    qualify_native_complete_insert_recovery, qualify_native_delete_recovery,
    qualify_native_eof_insert_recovery, qualify_native_insert_recovery,
    qualify_native_omitted_insert_recovery, qualify_native_query_expression_edits_recovery,
    qualify_native_query_expression_recovery, qualify_native_table_drop_recovery,
    qualify_native_table_rename_recovery, qualify_native_update_recovery,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::inspect_v3;
use tablecraft_io::journal::{
    RecoveryOutcome, filesystem::OfflineJournalPair, inspect_image, inspect_v1, inspect_v2,
};

impl OfflineWriteSession {
    /// Associate an explicitly selected active bundle with this retained target,
    /// replan its original normal intent, then recover/retire/clean it once.
    pub fn recover_update(
        mut self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Receipt), Failure> {
        let _span = tracing::debug_span!("access.offline.recover").entered();
        if let Err(cause) = self.open_pair(bundle, false, budget, cancelled) {
            return Err(self.fail(Phase::Setup, cause));
        }
        self.recover_pending(budget, cancelled)
    }

    /// Explicit recovery of the SAME retained active pair. Known completion is
    /// never replayed; finalization failures need diagnosis/confirmation instead.
    pub fn recover_pending(
        mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Receipt), Failure> {
        let _span = tracing::debug_span!("access.offline.recover_pending").entered();
        let mut schema = None;
        let mut query = None;
        let mut database_options = None;
        let result = (|| {
            let slot = self.slot_mut()?;
            if slot.current.is_some() || slot.state != SessionState::RetainedJournal {
                return Err(Cause::InvalidState);
            }
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            if pair.known_outcome().is_some() {
                return Err(Cause::InvalidState);
            }
            let intent = MutationIntent::decode(pair.intent_bytes(), budget, cancelled)?;
            schema = intent.schema_change();
            query = intent.query_change();
            database_options = intent.database_options_change();
            match intent {
                #[cfg(feature = "ace16-create")]
                MutationIntent::DatabaseOptions(intent) => {
                    super::database_options::recovery::recover(
                        pair, intent, slot.scope, budget, cancelled,
                    )
                }
                MutationIntent::QueryExpressionEdits(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_query_expression_edits_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::QueryExpressionEditsIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::QueryExpression(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_query_expression_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::QueryExpressionQualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddColumn(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::AddColumnIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::DefaultZeroAddColumn(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_default_zero_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::DefaultZeroAddColumnIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::RepeatedDefaultZeroAddColumn(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_repeated_default_zero_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::RepeatedDefaultZeroAddColumnIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAdditionAfterOff(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_after_off_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionAfterOffIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAdditionAfterOffHistory(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_after_off_history_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionAfterOffHistoryIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::RetainedTypedFieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::RetainedTypedFieldAdditionIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::UnadornedFieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::UnadornedFieldAdditionIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::DeclaredFieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::DeclaredFieldAdditionIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::CurrentFieldAdditionIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::CurrentFieldAddition(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_current_field_addition_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::CurrentFieldAdditionIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldDrop(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FieldDropIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldDrop(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_drop_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldDropIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldRename(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FieldRenameIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldRename(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_rename_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldRenameIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedTableDrop(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::TableDropIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::TableDrop(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_table_drop_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::TableDropIntent)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::TableRename(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_table_rename_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FirstTable(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let qualified = if intent.is_native() {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_native_recovery(checked, slot.scope, budget, cancelled)
                    } else {
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        qualify_native_first_table_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                    }
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::IndexCreation(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::IndexCreationIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FreshIndexCreation(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FreshIndexCreationIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::NumericIndexCreation(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::NumericIndexCreationIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::IndexDrop(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::IndexDropIntent)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::PopulatedTable(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_recovery_shared(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddTable(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let qualified = if intent.is_native() {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_native_recovery(checked, slot.scope, budget, cancelled)
                    } else if intent.source_profile() == AddTableSourceProfile::Retained {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                    } else {
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        qualify_native_add_table_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                    }
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::CompleteInsert(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_image(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_complete_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::EofInsert(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_eof_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedInsert(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::OmittedInsert(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_omitted_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::ClockInsert(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_clock_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Insert(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedDelete(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Delete(intent) => {
                    let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_delete_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedUpdate(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .recover_retained(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Update(intent) => {
                    if intent.profile() == NativeUpdateProfile::Ace16OrdinaryEofAllocation {
                        let (mut target, mut journal) =
                            pair.resizable_storage().map_err(Cause::Pair)?;
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        let qualified = qualify_native_allocation_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                        .map_err(Cause::Qualification)?;
                        qualified
                            .recover(&mut target, budget, cancelled)
                            .map_err(Cause::Recovery)
                    } else {
                        let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
                        let checked =
                            inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        let qualified = qualify_native_update_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                        .map_err(Cause::Qualification)?;
                        qualified
                            .recover(&mut target, budget, cancelled)
                            .map_err(Cause::Recovery)
                    }
                }
            }
        })();

        match result {
            Ok(outcome) => self.finish(
                Receipt {
                    kind: CompletionKind::Recovered,
                    retirement: None,
                    cleanup: None,
                    known: KnownOutcome::Native(outcome),
                    matched: None,
                    changed: None,
                    schema: if outcome == RecoveryOutcome::Committed {
                        schema
                    } else {
                        None
                    },
                    query: if outcome == RecoveryOutcome::Committed {
                        query
                    } else {
                        None
                    },
                    database_options: if outcome == RecoveryOutcome::Committed {
                        database_options
                    } else {
                        None
                    },
                },
                outcome,
                false,
            ),
            Err(cause) => Err(self.fail(Phase::Recovery, cause)),
        }
    }

    /// Confirm an intact retired bundle using write-denied storage. The receipt
    /// reports current Original/Replacement, never historical rollback/replay.
    /// Missing or partial garbage refuses without an active-recovery fallback.
    pub fn confirm_retired(
        mut self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Receipt), Failure> {
        let _span = tracing::debug_span!("access.offline.confirm_retired").entered();
        if let Err(cause) = self.open_pair(bundle, true, budget, cancelled) {
            return Err(self.fail(Phase::Setup, cause));
        }
        let mut schema = None;
        let mut query = None;
        let mut database_options = None;
        let result = (|| {
            let slot = self.slot_mut()?;
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            let intent = MutationIntent::decode(pair.intent_bytes(), budget, cancelled)?;
            schema = intent.schema_change();
            query = intent.query_change();
            database_options = intent.database_options_change();
            let (mut target, mut journal) = pair.confirmation_storage().map_err(Cause::Pair)?;
            match intent {
                #[cfg(feature = "ace16-create")]
                MutationIntent::DatabaseOptions(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent.qualify(checked, slot.scope, budget, cancelled)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::QueryExpressionEdits(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_query_expression_edits_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::QueryExpressionEditsIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::QueryExpression(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_query_expression_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::QueryExpressionQualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddColumn(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::AddColumnIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::DefaultZeroAddColumn(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_default_zero_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::DefaultZeroAddColumnIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::RepeatedDefaultZeroAddColumn(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_repeated_default_zero_add_column_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::RepeatedDefaultZeroAddColumnIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAddition(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAdditionAfterOff(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_after_off_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionAfterOffIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldAdditionAfterOffHistory(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_addition_after_off_history_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldAdditionAfterOffHistoryIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::RetainedTypedFieldAddition(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::RetainedTypedFieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::UnadornedFieldAddition(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::UnadornedFieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::DeclaredFieldAddition(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::DeclaredFieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldAddition(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::CurrentFieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::CurrentFieldAddition(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_current_field_addition_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::CurrentFieldAdditionIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldDrop(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FieldDropIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldDrop(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_drop_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldDropIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedFieldRename(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FieldRenameIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FieldRename(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_field_rename_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::FieldRenameIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedTableDrop(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::TableDropIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::TableDrop(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_table_drop_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::TableDropIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::TableRename(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_table_rename_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FirstTable(intent) => {
                    let qualified = if intent.is_native() {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_native_recovery(checked, slot.scope, budget, cancelled)
                    } else {
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        qualify_native_first_table_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                    }
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::IndexCreation(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::IndexCreationIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::FreshIndexCreation(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::FreshIndexCreationIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::NumericIndexCreation(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::NumericIndexCreationIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::IndexDrop(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::IndexDropIntent)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::PopulatedTable(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_recovery_shared(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddTable(intent) => {
                    let qualified = if intent.is_native() {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_native_recovery(checked, slot.scope, budget, cancelled)
                    } else if intent.source_profile() == AddTableSourceProfile::Retained {
                        let checked =
                            inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        intent.qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                    } else {
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        qualify_native_add_table_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                    }
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::CompleteInsert(intent) => {
                    let checked =
                        inspect_image(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_complete_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::EofInsert(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_eof_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedInsert(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::OmittedInsert(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_omitted_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::ClockInsert(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_clock_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Insert(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_insert_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedDelete(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Delete(intent) => {
                    let checked =
                        inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_delete_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::RetainedUpdate(intent) => {
                    let checked =
                        inspect_v3(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = intent
                        .qualify_retained_recovery(checked, slot.scope, budget, cancelled)
                        .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                MutationIntent::Update(intent) => {
                    if intent.profile() == NativeUpdateProfile::Ace16OrdinaryEofAllocation {
                        let checked =
                            inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        let qualified = qualify_native_allocation_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                        .map_err(Cause::Qualification)?;
                        qualified
                            .confirm_completed(&mut target, budget, cancelled)
                            .map_err(Cause::Recovery)
                    } else {
                        let checked =
                            inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                        let qualified = qualify_native_update_recovery(
                            checked, intent, slot.scope, budget, cancelled,
                        )
                        .map_err(Cause::Qualification)?;
                        qualified
                            .confirm_completed(&mut target, budget, cancelled)
                            .map_err(Cause::Recovery)
                    }
                }
            }
        })();

        match result {
            Ok(outcome) => {
                let image = match outcome {
                    RecoveryOutcome::Committed => VerifiedImage::Replacement,
                    RecoveryOutcome::RolledBack => VerifiedImage::Original,
                };
                self.finish(
                    Receipt {
                        kind: CompletionKind::Confirmed,
                        retirement: None,
                        cleanup: None,
                        known: KnownOutcome::Verified(image),
                        matched: None,
                        changed: None,
                        schema: if image == VerifiedImage::Replacement {
                            schema
                        } else {
                            None
                        },
                        query: if image == VerifiedImage::Replacement {
                            query
                        } else {
                            None
                        },
                        database_options: if image == VerifiedImage::Replacement {
                            database_options
                        } else {
                            None
                        },
                    },
                    outcome,
                    true,
                )
            }
            Err(cause) => Err(self.fail(Phase::Confirmation, cause)),
        }
    }

    fn open_pair(
        &mut self,
        bundle: &Path,
        retired: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        self.require_ready()?;
        let slot = self.slot_mut()?;
        slot.current = None;
        let target = slot.target.take().ok_or(Cause::InvalidState)?;
        let limit = tablecraft_ace::NativeIndexDropIntent::MAX_ENCODED_BYTES
            .max(tablecraft_ace::NativeFreshIndexCreateIntent::MAX_ENCODED_BYTES)
            .max(tablecraft_ace::NativeNumericIndexCreateIntent::MAX_ENCODED_BYTES)
            .max(NativeQueryExpressionIntent::MAX_ENCODED_BYTES)
            .max(NativeQueryExpressionEditsIntent::MAX_ENCODED_BYTES)
            .max(NativeTableRenameIntent::MAX_ENCODED_BYTES)
            .max(NativeTableDropIntent::MAX_ENCODED_BYTES)
            .max(NativeUpdateIntent::MAX_ENCODED_BYTES)
            .max(NativeFittedEditIntent::MAX_RETAINED_ENCODED_BYTES)
            .max(NativeDeleteIntent::MAX_ENCODED_BYTES)
            .max(NativeDeleteIntent::MAX_CURRENT_ENCODED_BYTES)
            .max(NativeDeleteIntent::MAX_RETAINED_ENCODED_BYTES)
            .max(NativeInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeOmittedInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeOmittedInsertIntent::MAX_CURRENT_ENCODED_BYTES)
            .max(NativeClockInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeEofInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeCompleteInsertIntent::MAX_ENCODED_BYTES) as u64;
        #[cfg(feature = "ace16-create")]
        let limit = limit
            .max(tablecraft_ace::NativeRetainedTypedFieldAdditionIntent::MAX_ENCODED_BYTES as u64)
            .max(tablecraft_ace::NativeDeclaredFieldAdditionIntent::MAX_ENCODED_BYTES as u64)
            .max(tablecraft_ace::NativeUnadornedFieldAdditionIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeOmittedInsertIntent::MAX_RETAINED_ENCODED_BYTES as u64)
            .max(NativeFirstTableIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFirstTableIntent::MAX_NATIVE_ENCODED_BYTES as u64)
            .max(NativePopulatedAddTableIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeAddTableIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeAddTableIntent::MAX_NATIVE_ENCODED_BYTES as u64)
            .max(NativeAddColumnIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeDefaultZeroAddColumnIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeRepeatedDefaultZeroAddColumnIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldAdditionIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldAdditionAfterOffIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldAdditionAfterOffHistoryIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeCurrentFieldAdditionIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldRenameIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldDropIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeDatabaseOptionsIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeFieldHistoryDatabaseOptionsIntent::MAX_ENCODED_BYTES as u64);
        let result = if retired {
            OfflineJournalPair::open_retired(target, bundle, limit, budget, cancelled)
        } else {
            OfflineJournalPair::open_existing(target, bundle, limit, budget, cancelled)
        };
        match result {
            Ok(pair) => {
                slot.pair = Some(pair);
                slot.state = SessionState::RetainedJournal;
                Ok(())
            }
            Err(error) => {
                slot.setup = Some(error);
                slot.state = SessionState::IncompleteSetup;
                Err(Cause::Setup)
            }
        }
    }
}
