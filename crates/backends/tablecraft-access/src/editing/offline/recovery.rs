use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt,
    SessionState, VerifiedImage,
};
use std::path::Path;
use tablecraft_ace::{NativeUpdateIntent, qualify_native_update_recovery};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair, inspect_v1};

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
        let result = (|| {
            let slot = self.slot_mut()?;
            if slot.current.is_some() || slot.state != SessionState::RetainedJournal {
                return Err(Cause::InvalidState);
            }
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            if pair.known_outcome().is_some() {
                return Err(Cause::InvalidState);
            }
            let intent = NativeUpdateIntent::decode(pair.intent_bytes(), budget, cancelled)
                .map_err(Cause::Intent)?;
            let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
            let checked = inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
            let qualified =
                qualify_native_update_recovery(checked, intent, slot.scope, budget, cancelled)
                    .map_err(Cause::Qualification)?;
            qualified
                .recover(&mut target, budget, cancelled)
                .map_err(Cause::Recovery)
        })();

        match result {
            Ok(outcome) => self.finish(
                Receipt {
                    kind: CompletionKind::Recovered,
                    known: KnownOutcome::Native(outcome),
                    matched: None,
                    changed: None,
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
        let result = (|| {
            let slot = self.slot_mut()?;
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            let intent = NativeUpdateIntent::decode(pair.intent_bytes(), budget, cancelled)
                .map_err(Cause::Intent)?;
            let (mut target, mut journal) = pair.confirmation_storage().map_err(Cause::Pair)?;
            let checked = inspect_v1(&mut journal, budget, cancelled).map_err(Cause::Recovery)?;
            let qualified =
                qualify_native_update_recovery(checked, intent, slot.scope, budget, cancelled)
                    .map_err(Cause::Qualification)?;
            qualified
                .confirm_completed(&mut target, budget, cancelled)
                .map_err(Cause::Recovery)
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
                        known: KnownOutcome::Verified(image),
                        matched: None,
                        changed: None,
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
        let limit = NativeUpdateIntent::MAX_ENCODED_BYTES as u64;
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
