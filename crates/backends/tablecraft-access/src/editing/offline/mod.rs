//! Explicit offline normal edits over one retained exclusive target descriptor.
//! Hosts must exclude noncooperating writers and unresolved external journals.
//! Native planners retain their own admission; shared-file coordination is separate.
mod apply;
mod delete;
mod error;
mod insert;
mod plan;
mod recovery;
mod table;
mod update;

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;

pub use delete::StagedDelete;
pub use error::{Cause, Failure, OpenError, Phase};
pub use insert::{OfflineTableInserter, StagedInsert};
pub use table::OfflineTableEditor;
pub use update::StagedUpdate;

use std::path::Path;
use tablecraft_ace::{InspectError, NativeImage};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{
    RecoveryOutcome,
    filesystem::{OfflineJournalPair, OfflineTarget, PairError, PairSetupFailure, PairState},
};
use tablecraft_model::CatalogScope;

/// Kind of acknowledged completion, separate from cleanup and release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    /// A newly planned normal edit was committed and its bundle retired/cleaned.
    Applied,
    /// An active qualified journal was explicitly recovered and retired/cleaned.
    Recovered,
    /// An intact retired journal verified the current complete image without replay.
    Confirmed,
}

/// Currently verified image, not a claim about historical replay or rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifiedImage {
    /// The current target exactly matches checked BEFORE, with acknowledged barriers.
    Original,
    /// The current target exactly matches checked AFTER, with acknowledged barriers.
    Replacement,
}

/// Known fact for the current retained operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownOutcome {
    /// An acknowledged native commit or explicit recovery result.
    Native(RecoveryOutcome),
    /// A current-image observation, never a historical commit/rollback claim.
    Verified(VerifiedImage),
}

/// Allocation-free completion data. It is not stdout or a native-save grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    kind: CompletionKind,
    known: KnownOutcome,
    matched: Option<usize>,
    changed: Option<usize>,
}

impl Receipt {
    /// Whether this receipt describes an apply, recovery or current-image confirmation.
    pub const fn kind(self) -> CompletionKind {
        self.kind
    }

    /// Native operation outcome; confirmation deliberately returns None.
    pub const fn outcome(self) -> Option<RecoveryOutcome> {
        match self.known {
            KnownOutcome::Native(value) => Some(value),
            _ => None,
        }
    }

    /// Verified current image for confirmation; apply/recovery return None.
    pub const fn verified_image(self) -> Option<VerifiedImage> {
        match self.known {
            KnownOutcome::Verified(value) => Some(value),
            _ => None,
        }
    }

    /// Original matched rows for apply; recovery/confirmation do not invent counts.
    pub const fn matched_rows(self) -> Option<usize> {
        self.matched
    }

    /// Effective changed rows for apply; recovery/confirmation return None.
    pub const fn changed_rows(self) -> Option<usize> {
        self.changed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Capability state controlling normal access, independent of namespace phase.
pub enum SessionState {
    /// The same exclusive target is available for a new normal plan.
    Ready,
    /// A complete capture failed; explicit refresh is required before another plan.
    RefreshRequired,
    /// An active or terminal journal is retained; normal planning is blocked.
    RetainedJournal,
    /// Setup capabilities remain retained for explicit diagnosis.
    IncompleteSetup,
    /// An unlock may have taken effect; no further storage/release attempt is offered.
    ReleaseUncertain,
    /// Resources acknowledged release; no normal access is offered.
    Released,
}

struct Resources {
    target: Option<OfflineTarget>,
    pair: Option<OfflineJournalPair>,
    setup: Option<PairSetupFailure>,
    scope: CatalogScope,
    state: SessionState,
    current: Option<KnownOutcome>,
    last: Option<Receipt>,
}

/// One prepaid resource slot keeps owned failures small without allocating after
/// effects. Failure retains this SAME owner. Dropping only performs handle Drop;
/// it never deletes journals, retries SQL or acknowledges normal access.
pub struct OfflineWriteSession {
    resources: Vec<Resources>,
}

impl OfflineWriteSession {
    /// Acquire one explicit existing target after paying the retained-owner allocation.
    /// The caller supplies catalog scope and excludes noncooperating writers and
    /// unresolved journals outside this session. No native mutation is authorized.
    pub fn open(
        path: &Path,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, OpenError> {
        let _span = tracing::debug_span!("access.offline.open").entered();
        check(budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<Resources>() as u64)
            .map_err(InspectError::from)?;
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        let target = OfflineTarget::open(path, budget, cancelled)?;
        resources.push(Resources {
            target: Some(target),
            pair: None,
            setup: None,
            scope,
            state: SessionState::Ready,
            current: None,
            last: None,
        });

        Ok(Self { resources })
    }

    fn slot(&self) -> Result<&Resources, Cause> {
        self.resources.first().ok_or(Cause::InvalidState)
    }

    fn slot_mut(&mut self) -> Result<&mut Resources, Cause> {
        self.resources.first_mut().ok_or(Cause::InvalidState)
    }

    /// Current session capability state; inspect pair state separately.
    pub fn state(&self) -> SessionState {
        self.resources
            .first()
            .map_or(SessionState::Released, |slot| slot.state)
    }

    /// Current operation fact. A failed later action does not erase last_completed.
    pub fn current_outcome(&self) -> Option<KnownOutcome> {
        self.resources.first().and_then(|slot| slot.current)
    }

    /// Earlier successfully finalized operation, retained across a later failure.
    pub fn last_completed(&self) -> Option<Receipt> {
        self.resources.first().and_then(|slot| slot.last)
    }

    /// Exact retained setup failure/capabilities, including incomplete namespaces.
    pub fn setup_failure(&self) -> Option<&PairSetupFailure> {
        self.resources.first().and_then(|slot| slot.setup.as_ref())
    }

    /// Exact retained namespace/release state, separate from native completion.
    pub fn pair_state(&self) -> Option<PairState> {
        self.resources
            .first()
            .and_then(|slot| slot.pair.as_ref())
            .map(OfflineJournalPair::state)
    }

    /// Retained namespace phase, including the phase before an uncertain release.
    pub fn pair_namespace_state(&self) -> Option<PairState> {
        self.resources
            .first()
            .and_then(|slot| slot.pair.as_ref())
            .map(OfflineJournalPair::namespace_state)
    }

    fn require_ready(&self) -> Result<(), Cause> {
        let slot = self.slot()?;
        if slot.state != SessionState::Ready
            || slot.target.is_none()
            || slot.pair.is_some()
            || slot.setup.is_some()
        {
            return Err(Cause::InvalidState);
        }
        Ok(())
    }

    /// Explicitly acknowledge a new complete read after a capture failure.
    /// This never repairs, discovers or replays a retained journal.
    pub fn refresh(
        mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Failure> {
        let _span = tracing::debug_span!("access.offline.refresh").entered();
        let result = (|| {
            let slot = self.slot_mut()?;
            if !matches!(
                slot.state,
                SessionState::Ready | SessionState::RefreshRequired
            ) || slot.pair.is_some()
                || slot.setup.is_some()
            {
                return Err(Cause::InvalidState);
            }
            let target = slot.target.as_mut().ok_or(Cause::InvalidState)?;
            slot.state = SessionState::RefreshRequired;
            NativeImage::capture(target, slot.scope, budget, cancelled).map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            Ok(())
        })();

        match result {
            Ok(()) => Ok(self),
            Err(cause) => Err(self.fail(Phase::Refresh, cause)),
        }
    }

    /// Explicit unlock. No bundle entry is removed or repaired. Incomplete setup
    /// releases its retained locks once, preserving namespace entries for diagnosis.
    /// Every unlock error retains owners and excludes further release/storage.
    pub fn release(self) -> Result<(), Failure> {
        self.release_with_setup(PairSetupFailure::release_resources)
    }

    fn release_with_setup(
        mut self,
        release_setup: impl FnOnce(&mut PairSetupFailure) -> Result<(), PairError>,
    ) -> Result<(), Failure> {
        let _span = tracing::debug_span!("access.offline.release").entered();
        let result = (|| {
            let slot = self.slot_mut()?;
            if matches!(
                slot.state,
                SessionState::ReleaseUncertain | SessionState::Released
            ) {
                return Err(Cause::InvalidState);
            }
            slot.state = SessionState::ReleaseUncertain;
            if let Some(setup) = slot.setup.as_mut() {
                release_setup(setup).map_err(Cause::ReleaseSetup)?;
            } else if let Some(pair) = slot.pair.as_mut() {
                pair.release().map_err(Cause::ReleasePair)?;
            } else {
                slot.target
                    .as_mut()
                    .ok_or(Cause::InvalidState)?
                    .release()
                    .map_err(Cause::Io)?;
            }
            slot.state = SessionState::Released;
            Ok(())
        })();

        match result {
            Ok(()) => Ok(()),
            Err(cause) => Err(self.fail(Phase::Release, cause)),
        }
    }

    fn fail(self, phase: Phase, cause: Cause) -> Failure {
        Failure::new(self, phase, cause)
    }

    fn finish(
        mut self,
        receipt: Receipt,
        outcome: RecoveryOutcome,
        confirmation: bool,
    ) -> Result<(Self, Receipt), Failure> {
        let result = (|| {
            let slot = self.slot_mut()?;
            slot.current = Some(receipt.known);
            let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
            if confirmation {
                pair.acknowledge_retired(outcome)
                    .map_err(Cause::Finalization)?;
            } else {
                pair.retire(outcome).map_err(Cause::Finalization)?;
            }
            pair.cleanup_retired().map_err(Cause::Finalization)?;
            let pair = slot.pair.take().ok_or(Cause::InvalidState)?;
            match pair.into_target_after_clean() {
                Ok(target) => slot.target = Some(target),
                Err(failure) => {
                    let (cause, pair) = failure.into_parts();
                    if pair.state() == PairState::ReleaseUncertain {
                        slot.state = SessionState::ReleaseUncertain;
                    }
                    slot.pair = Some(pair);
                    return Err(Cause::Transfer(cause));
                }
            }
            slot.last = Some(receipt);
            slot.current = None;
            slot.state = SessionState::Ready;
            Ok(())
        })();

        match result {
            Ok(()) => Ok((self, receipt)),
            Err(cause) => Err(self.fail(Phase::Finalization, cause)),
        }
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)
}
