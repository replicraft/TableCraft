//! Retain one target and one private flat sidecar through native completion.
use super::{
    FinalizationError, FinalizationPhase, OfflineTarget, PairError, PairState,
    RetiredCleanupPolicy, RetirementGuarantee, TargetLockState,
    setup::{self, Sidecar},
    storage::{PairedStorage, ResizablePairedTarget},
};
use crate::{
    IoError,
    journal::{
        RecoveryOutcome,
        flat_retirement::{NamespaceOps, TerminalPermit},
    },
    progress::check,
    windows_fs,
};
use std::{error::Error, fmt, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};

// Fixed rename buffer, four identity checks, flush, one explicit flush retry and
// retained-handle closes. Reserve before setup/native effects, not after commit.
const TERMINAL_STEPS: u64 = 4096;

pub struct PairSetupFailure {
    cause: PairError,
    target: OfflineTarget,
    incomplete: Option<Sidecar>,
    release_attempted: bool,
}

impl PairSetupFailure {
    pub fn cause(&self) -> &PairError {
        &self.cause
    }

    pub fn incomplete_setup(&self) -> bool {
        self.incomplete.is_some()
    }

    pub fn release_resources(&mut self) -> Result<(), PairError> {
        if self.release_attempted {
            return Err(PairError::InvalidState);
        }
        self.release_attempted = true;
        self.target.release()?;
        if let Some(sidecar) = &mut self.incomplete {
            sidecar.close_resources()?;
        }
        Ok(())
    }

    pub fn into_target(self) -> OfflineTarget {
        self.target
    }
}

impl fmt::Debug for PairSetupFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("PairSetupFailure")
            .field("cause", &self.cause)
            .field(
                "setup_phase",
                &self.incomplete.as_ref().map(|sidecar| sidecar.phase),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PairSetupFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "journal sidecar setup failed: {}", self.cause)
    }
}

impl Error for PairSetupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// The exact retained files, not permission to execute a native database edit.
pub struct OfflineJournalPair {
    target: OfflineTarget,
    sidecar: Sidecar,
    state: PairState,
    known: Option<RecoveryOutcome>,
    before_release: Option<PairState>,
    terminal: TerminalPermit,
}

fn reserve(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), PairError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, TERMINAL_STEPS)
        .map_err(IoError::from)?;
    Ok(())
}

impl OfflineJournalPair {
    #[allow(clippy::result_large_err)] // Retain all owners inline without allocating on failure.
    pub fn create(
        target: OfflineTarget,
        path: &Path,
        intent: Vec<u8>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        Self::create_with_intent_limit(
            target,
            path,
            intent,
            setup::DEFAULT_MAX_INTENT_BYTES,
            budget,
            cancelled,
        )
    }

    /// Publish with a host-selected opaque intent allowance, checked before any
    /// namespace setup. No native action is interpreted by this filesystem owner.
    #[allow(clippy::result_large_err)] // Retain all owners inline without allocating on failure.
    pub fn create_with_intent_limit(
        mut target: OfflineTarget,
        path: &Path,
        intent: Vec<u8>,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        if let Err(cause) = reserve(budget, cancelled) {
            return Err(PairSetupFailure {
                cause,
                target,
                incomplete: None,
                release_attempted: false,
            });
        }
        match setup::create(
            &mut target,
            path,
            intent,
            maximum_intent_bytes,
            budget,
            cancelled,
        ) {
            Ok(sidecar) => Ok(Self {
                target,
                sidecar,
                state: PairState::Ready,
                known: None,
                before_release: None,
                terminal: TerminalPermit::reserved(false),
            }),
            Err(failed) => Err(PairSetupFailure {
                cause: failed.cause,
                target,
                incomplete: failed.sidecar,
                release_attempted: false,
            }),
        }
    }

    #[allow(clippy::result_large_err)] // Retain the target and partially opened sidecar.
    pub fn open_existing(
        target: OfflineTarget,
        path: &Path,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        Self::open(target, path, maximum_intent_bytes, false, budget, cancelled)
    }

    #[allow(clippy::result_large_err)] // Retain the target and partially opened sidecar.
    pub fn open_retired(
        target: OfflineTarget,
        path: &Path,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        Self::open(target, path, maximum_intent_bytes, true, budget, cancelled)
    }

    #[allow(clippy::result_large_err)] // Failure cannot depend on another allocation.
    fn open(
        target: OfflineTarget,
        path: &Path,
        maximum_intent_bytes: u64,
        retired: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        if let Err(cause) = reserve(budget, cancelled) {
            return Err(PairSetupFailure {
                cause,
                target,
                incomplete: None,
                release_attempted: false,
            });
        }
        match setup::reopen(
            &target,
            path,
            maximum_intent_bytes,
            retired,
            budget,
            cancelled,
        ) {
            Ok(sidecar) => Ok(Self {
                target,
                sidecar,
                state: if retired {
                    PairState::RetiredVerificationRequired
                } else {
                    PairState::RecoveryRequired
                },
                known: None,
                before_release: None,
                terminal: TerminalPermit::reserved(retired),
            }),
            Err(failed) => Err(PairSetupFailure {
                cause: failed.cause,
                target,
                incomplete: failed.sidecar,
                release_attempted: false,
            }),
        }
    }

    pub fn intent_bytes(&self) -> &[u8] {
        &self.sidecar.intent
    }

    fn devices(
        &mut self,
        writable: bool,
    ) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        let prefix = self.sidecar.binding()?.journal_offset()?;
        let capacity = self.sidecar.binding()?.journal_capacity()?;
        let target = PairedStorage::target(self.target.file_mut()?, writable);
        let journal =
            PairedStorage::journal(&mut self.sidecar.storage, prefix, capacity, writable)?;
        Ok((target, journal))
    }

    pub fn storage(&mut self) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        if !matches!(self.state, PairState::Ready | PairState::RecoveryRequired)
            || self.known.is_some()
        {
            return Err(PairError::InvalidState);
        }
        self.state = PairState::RecoveryRequired;
        self.devices(true)
    }

    pub fn resizable_storage(
        &mut self,
    ) -> Result<(ResizablePairedTarget<'_>, PairedStorage<'_>), PairError> {
        let (target, journal) = self.storage()?;
        Ok((ResizablePairedTarget::new(target), journal))
    }

    pub fn confirmation_storage(
        &mut self,
    ) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        if self.state != PairState::RetiredVerificationRequired || self.known.is_some() {
            return Err(PairError::InvalidState);
        }
        self.devices(false)
    }

    pub fn note_outcome(&mut self, outcome: RecoveryOutcome) -> Result<(), PairError> {
        if self.state != PairState::RecoveryRequired || self.known.is_some() {
            return Err(PairError::InvalidState);
        }
        self.known = Some(outcome);
        self.state = PairState::OutcomeRecorded;
        Ok(())
    }

    fn finalization(&self, phase: FinalizationPhase, cause: PairError) -> FinalizationError {
        FinalizationError {
            phase,
            outcome: self.known,
            cause,
        }
    }

    pub fn retire(&mut self, outcome: RecoveryOutcome) -> Result<(), FinalizationError> {
        self.terminal.retire(
            &mut self.state,
            &mut self.known,
            outcome,
            &mut Actual {
                target: &mut self.target,
                sidecar: &self.sidecar,
            },
        )
    }

    pub fn confirm_retirement(
        &mut self,
        outcome: RecoveryOutcome,
    ) -> Result<(), FinalizationError> {
        self.terminal.confirm(
            &mut self.state,
            self.known,
            outcome,
            &mut Actual {
                target: &mut self.target,
                sidecar: &self.sidecar,
            },
        )
    }

    pub fn acknowledge_retired(
        &mut self,
        outcome: RecoveryOutcome,
    ) -> Result<(), FinalizationError> {
        self.terminal.acknowledge(
            &mut self.state,
            &mut self.known,
            outcome,
            &mut Actual {
                target: &mut self.target,
                sidecar: &self.sidecar,
            },
        )
    }

    pub const fn cleanup_policy(&self) -> RetiredCleanupPolicy {
        RetiredCleanupPolicy::RetainIntact
    }

    pub fn retirement_guarantee(&self) -> Option<RetirementGuarantee> {
        (self.namespace_state() == PairState::Retired && self.known.is_some())
            .then_some(RetirementGuarantee::NtfsWriteThroughRename)
    }

    /// This platform deliberately keeps the retired artifact intact. A delete
    /// request cannot be reported as Unix unlink-plus-parent-barrier cleanup.
    pub fn cleanup_retired(&mut self) -> Result<(), FinalizationError> {
        Err(self.finalization(FinalizationPhase::State, PairError::InvalidState))
    }

    pub const fn state(&self) -> PairState {
        self.state
    }

    pub fn namespace_state(&self) -> PairState {
        self.before_release.unwrap_or(self.state)
    }

    pub const fn known_outcome(&self) -> Option<RecoveryOutcome> {
        self.known
    }

    pub fn release(&mut self) -> Result<(), PairError> {
        if matches!(
            self.state,
            PairState::ReleaseUncertain | PairState::Released
        ) {
            return Err(PairError::InvalidState);
        }
        self.before_release = Some(self.state);
        self.state = PairState::ReleaseUncertain;
        self.target.release()?;
        self.sidecar.close_resources()?;
        self.state = PairState::Released;
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub fn into_target_after_clean(self) -> Result<OfflineTarget, TargetTransferFailure> {
        Err(TargetTransferFailure {
            cause: PairError::InvalidState,
            pair: self,
        })
    }

    #[allow(clippy::result_large_err)]
    pub fn into_target_after_retirement(mut self) -> Result<OfflineTarget, TargetTransferFailure> {
        if self.state != PairState::Retired
            || self.known.is_none()
            || self.target.lock_state() != TargetLockState::Held
        {
            return Err(TargetTransferFailure {
                cause: PairError::InvalidState,
                pair: self,
            });
        }
        self.before_release = Some(self.state);
        self.state = PairState::ReleaseUncertain;
        if let Err(cause) = self.sidecar.close_resources() {
            return Err(TargetTransferFailure { cause, pair: self });
        }
        Ok(self.target)
    }
}

/// Actual retained descriptors; no path reopening or cached identity substitute.
struct Actual<'a> {
    target: &'a mut OfflineTarget,
    sidecar: &'a Sidecar,
}

impl NamespaceOps for Actual<'_> {
    fn verify(&mut self) -> Result<(), PairError> {
        let expected = self.sidecar.binding()?.objects;
        let actual = [
            (
                windows_fs::identity(self.target.file_mut()?)?,
                expected.target,
            ),
            (
                windows_fs::identity(self.target.parent()?)?,
                expected.target_parent,
            ),
            (
                windows_fs::identity(self.sidecar.file()?)?,
                expected.sidecar,
            ),
            (
                windows_fs::identity(self.sidecar.parent()?)?,
                expected.sidecar_parent,
            ),
        ];
        if actual
            .into_iter()
            .any(|(identity, expected)| super::binding_identity(identity) != expected)
        {
            return Err(PairError::IdentityMismatch);
        }
        Ok(())
    }

    fn rename(&mut self) -> Result<(), PairError> {
        windows_fs::rename_no_replace(
            self.sidecar.file()?,
            self.sidecar.parent()?,
            &self.sidecar.retired_name,
        )?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), PairError> {
        windows_fs::flush(self.sidecar.file()?)?;
        Ok(())
    }
}

#[must_use = "retain the pair; transfer failure does not imply rollback"]
pub struct TargetTransferFailure {
    cause: PairError,
    pair: OfflineJournalPair,
}

impl TargetTransferFailure {
    pub fn cause(&self) -> &PairError {
        &self.cause
    }

    pub fn pair(&self) -> &OfflineJournalPair {
        &self.pair
    }

    pub fn into_parts(self) -> (PairError, OfflineJournalPair) {
        (self.cause, self.pair)
    }
}

impl fmt::Debug for TargetTransferFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("TargetTransferFailure")
            .field("cause", &self.cause)
            .field("state", &self.pair.state())
            .field("known_outcome", &self.pair.known_outcome())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for TargetTransferFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "journal target transfer failed: {}", self.cause)
    }
}

impl Error for TargetTransferFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
