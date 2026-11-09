//! One checked V2 journal owner for selected old/new original options intents.
use super::{super::Cause, intent::Intent};
use tablecraft_ace::{
    QualifiedDatabaseOptionsRecovery, QualifiedFieldHistoryDatabaseOptionsRecovery,
    qualify_native_database_options_recovery,
    qualify_native_field_history_database_options_recovery,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    filesystem::OfflineJournalPair, inspect_v2,
};
use tablecraft_model::CatalogScope;

pub(in crate::editing::offline) enum Qualified<'a, J: DurableStorage> {
    Creator(QualifiedDatabaseOptionsRecovery<'a, J>),
    FieldHistory(QualifiedFieldHistoryDatabaseOptionsRecovery<'a, J>),
}

impl Intent {
    pub(in crate::editing::offline) fn qualify<'a, J: DurableStorage>(
        self,
        checked: CheckedV2<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Qualified<'a, J>, Cause> {
        match self {
            Self::Creator(intent) => {
                qualify_native_database_options_recovery(checked, intent, scope, budget, cancelled)
                    .map(Qualified::Creator)
                    .map_err(Cause::DatabaseOptionsIntent)
            }
            Self::FieldHistory(intent) => qualify_native_field_history_database_options_recovery(
                checked, intent, scope, budget, cancelled,
            )
            .map(Qualified::FieldHistory)
            .map_err(Cause::FieldHistoryDatabaseOptionsIntent),
        }
    }
}

impl<J: DurableStorage> Qualified<'_, J> {
    fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        match self {
            Self::Creator(qualified) => qualified.recover(target, budget, cancelled),
            Self::FieldHistory(qualified) => qualified.recover(target, budget, cancelled),
        }
    }

    pub(in crate::editing::offline) fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        match self {
            Self::Creator(qualified) => qualified.confirm_completed(target, budget, cancelled),
            Self::FieldHistory(qualified) => qualified.confirm_completed(target, budget, cancelled),
        }
    }
}

pub(in crate::editing::offline) fn recover(
    pair: &mut OfflineJournalPair,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoveryOutcome, Cause> {
    let (mut target, mut journal) = pair.resizable_storage().map_err(Cause::Pair)?;
    let checked = inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
    intent
        .qualify(checked, scope, budget, cancelled)?
        .recover(&mut target, budget, cancelled)
        .map_err(Cause::Recovery)
}
