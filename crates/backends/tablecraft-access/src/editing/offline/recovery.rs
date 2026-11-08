use super::plan::MutationIntent;
use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt,
    SessionState, VerifiedImage,
};
use std::path::Path;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    NativeAddTableIntent, NativeFirstTableIntent, qualify_native_add_table_recovery,
    qualify_native_first_table_recovery,
};
use tablecraft_ace::{
    NativeClockInsertIntent, NativeCompleteInsertIntent, NativeDeleteIntent, NativeEofInsertIntent,
    NativeInsertIntent, NativeOmittedInsertIntent, NativeTableRenameIntent, NativeUpdateIntent,
    NativeUpdateProfile, qualify_native_allocation_recovery, qualify_native_clock_insert_recovery,
    qualify_native_complete_insert_recovery, qualify_native_delete_recovery,
    qualify_native_eof_insert_recovery, qualify_native_insert_recovery,
    qualify_native_omitted_insert_recovery, qualify_native_table_rename_recovery,
    qualify_native_update_recovery,
};
use tablecraft_core::limits::ResourceBudget;
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
            match intent {
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
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_first_table_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .recover(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddTable(intent) => {
                    let (mut target, mut journal) =
                        pair.resizable_storage().map_err(Cause::Pair)?;
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_add_table_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
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
        let result = (|| {
            let slot = self.slot_mut()?;
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            let intent = MutationIntent::decode(pair.intent_bytes(), budget, cancelled)?;
            schema = intent.schema_change();
            let (mut target, mut journal) = pair.confirmation_storage().map_err(Cause::Pair)?;
            match intent {
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
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_first_table_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
                    .map_err(Cause::Qualification)?;
                    qualified
                        .confirm_completed(&mut target, budget, cancelled)
                        .map_err(Cause::Recovery)
                }
                #[cfg(feature = "ace16-create")]
                MutationIntent::AddTable(intent) => {
                    let checked =
                        inspect_v2(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
                    let qualified = qualify_native_add_table_recovery(
                        checked, intent, slot.scope, budget, cancelled,
                    )
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
        let limit = NativeTableRenameIntent::MAX_ENCODED_BYTES
            .max(NativeUpdateIntent::MAX_ENCODED_BYTES)
            .max(NativeDeleteIntent::MAX_ENCODED_BYTES)
            .max(NativeInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeOmittedInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeClockInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeEofInsertIntent::MAX_ENCODED_BYTES)
            .max(NativeCompleteInsertIntent::MAX_ENCODED_BYTES) as u64;
        #[cfg(feature = "ace16-create")]
        let limit = limit
            .max(NativeFirstTableIntent::MAX_ENCODED_BYTES as u64)
            .max(NativeAddTableIntent::MAX_ENCODED_BYTES as u64);
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
