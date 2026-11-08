//! Cold and retained recovery keep the exact V2 session and original TCDO request.
use super::super::Cause;
use tablecraft_ace::{NativeDatabaseOptionsIntent, qualify_native_database_options_recovery};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair, inspect_v2};
use tablecraft_model::CatalogScope;

pub(in crate::editing::offline) fn recover(
    pair: &mut OfflineJournalPair,
    intent: NativeDatabaseOptionsIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoveryOutcome, Cause> {
    let (mut target, mut journal) = pair.resizable_storage().map_err(Cause::Pair)?;
    let checked = inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
    let qualified =
        qualify_native_database_options_recovery(checked, intent, scope, budget, cancelled)
            .map_err(Cause::DatabaseOptionsIntent)?;
    qualified
        .recover(&mut target, budget, cancelled)
        .map_err(Cause::Recovery)
}
