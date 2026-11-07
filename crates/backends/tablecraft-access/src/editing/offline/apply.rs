//! Shared retained lifecycle; action-specific plans supply honest optional counts.
use super::plan::MutationPlan;
use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt,
    SchemaChange, SessionState,
};
use std::path::Path;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};

pub(super) fn apply(
    mut session: OfflineWriteSession,
    plan: MutationPlan,
    counts: (Option<usize>, Option<usize>),
    schema: Option<SchemaChange>,
    bundle: &Path,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(OfflineWriteSession, Receipt), Failure> {
    let _span = tracing::debug_span!("access.offline.apply").entered();
    let (matched, changed) = counts;
    let intent = match plan.encode_intent(budget, cancelled) {
        Ok(intent) => intent,
        Err(cause) => return Err(session.fail(Phase::Intent, cause)),
    };
    let setup = (|| {
        session.require_ready()?;
        let slot = session.slot_mut()?;
        let target = slot.target.take().ok_or(Cause::InvalidState)?;
        match OfflineJournalPair::create(target, bundle, intent, budget, cancelled) {
            Ok(pair) => {
                slot.pair = Some(pair);
                slot.state = SessionState::RetainedJournal;
            }
            Err(failure) => {
                slot.setup = Some(failure);
                slot.state = SessionState::IncompleteSetup;
                return Err(Cause::Setup);
            }
        }
        Ok(())
    })();

    if let Err(cause) = setup {
        return Err(session.fail(Phase::Setup, cause));
    }
    let result = (|| {
        let slot = session.slot_mut()?;
        let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
        let (mut target, mut journal) = pair.resizable_storage().map_err(Cause::Pair)?;
        plan.commit(&mut target, &mut journal, budget, cancelled)?;
        Ok(())
    })();

    if let Err(cause) = result {
        let phase = if matches!(cause, Cause::Commit(_)) {
            Phase::Commit
        } else {
            Phase::Prepare
        };
        return Err(session.fail(phase, cause));
    }
    session.finish(
        Receipt {
            kind: CompletionKind::Applied,
            known: KnownOutcome::Native(RecoveryOutcome::Committed),
            matched,
            changed,
            schema,
        },
        RecoveryOutcome::Committed,
        false,
    )
}
