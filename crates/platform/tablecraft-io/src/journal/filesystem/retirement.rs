use super::{
    BINDING_ENTRY, INTENT_ENTRY, JOURNAL_ENTRY, PairError, PairState, pair::retirement_name,
    reopen::ExistingBundle,
};
use crate::{IoError, journal::RecoveryOutcome, progress::check};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use rustix::fs::RenameFlags;
use rustix::fs::{self, AtFlags};
use std::{ffi::CStr, io::ErrorKind};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

// Fixed callbacks plus conservative byte work for formatting/copying the fixed
// retired name twice and validating its CStr four times. Variable active-name
// scans are separate. These reserves are consumed before namespace/target effects.
const CONTROL_CALLBACK_STEPS: u64 = 64;
const RETIRED_COMPONENT_BYTES: u64 = (super::pair::RETIRED_NAME_BYTES + 1) as u64;
const NAME_CONSTRUCTIONS: u64 = 2;
const FORMATTING_PASSES: u64 = 8;
const NAME_VALIDATIONS: u64 = 4;
const TERMINAL_STEPS: u64 = CONTROL_CALLBACK_STEPS
    + RETIRED_COMPONENT_BYTES * (NAME_CONSTRUCTIONS * FORMATTING_PASSES + NAME_VALIDATIONS);
const ACTIVE_NAME_USES: u64 = 3;

/// Namespace stage that failed after a separately known native outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalizationPhase {
    /// The active bundle was not safely retired or its outcome is unknown.
    Retirement,
    /// Rename succeeded, but its parent barrier was not acknowledged.
    RetirementBarrier,
    /// Durable retirement succeeded; only retired garbage may remain.
    GarbageCleanup,
    /// Caller attempted an operation outside its retained phase/permit.
    State,
}

/// Completion facts survive namespace errors; no variant authorizes replay.
#[derive(Debug, Error)]
#[error("journal finalization failed ({phase:?}); known outcome {outcome:?}: {cause}")]
pub struct FinalizationError {
    /// Failed lifecycle stage, without paths or native values.
    pub phase: FinalizationPhase,
    /// Acknowledged native result, independent of cleanup/barrier failure.
    pub outcome: Option<RecoveryOutcome>,
    /// Sanitized failure; retain the owned pair for explicit diagnosis.
    #[source]
    pub cause: PairError,
}

pub(super) struct TerminalPermit {
    retire: bool,
    cleanup: bool,
}

impl TerminalPermit {
    pub(super) fn reserve(
        maximum_name_bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        check(budget, cancelled)?;
        let steps = (maximum_name_bytes as u64)
            .checked_add(1)
            .and_then(|length| length.checked_mul(ACTIVE_NAME_USES))
            .and_then(|work| work.checked_add(TERMINAL_STEPS))
            .ok_or(IoError::Io(ErrorKind::InvalidInput))?;
        budget.charge(Resource::Steps, steps)?;
        Ok(Self {
            retire: true,
            cleanup: true,
        })
    }
}

enum RenameFailure {
    Collision(IoError),
    Uncertain(IoError),
}

trait NamespaceOps {
    fn verify_active(&mut self) -> Result<(), IoError>;
    fn rename(&mut self) -> Result<(), RenameFailure>;
    fn sync_parent(&mut self) -> Result<(), IoError>;
    fn cleanup(&mut self) -> Result<(), IoError>;
}

struct Actual<'a> {
    bundle: &'a ExistingBundle,
    retired: RetirementName,
}

struct RetirementName {
    bytes: [u8; super::pair::RETIRED_NAME_BYTES + 1],
}

impl RetirementName {
    fn new(bundle: &ExistingBundle) -> Self {
        let raw = retirement_name(&bundle.binding.nonce);
        let mut bytes = [0; super::pair::RETIRED_NAME_BYTES + 1];
        bytes[..raw.len()].copy_from_slice(&raw);
        Self { bytes }
    }

    fn name(&self) -> &CStr {
        CStr::from_bytes_with_nul(&self.bytes).expect("fixed ASCII retirement name")
    }
}

impl NamespaceOps for Actual<'_> {
    fn verify_active(&mut self) -> Result<(), IoError> {
        self.bundle
            ._parent
            .verify_child(self.bundle.name.as_c_str(), self.bundle.binding.bundle)?;
        for (name, identity) in [
            (BINDING_ENTRY, self.bundle.binding.binding),
            (INTENT_ENTRY, self.bundle.binding.intent),
            (JOURNAL_ENTRY, self.bundle.binding.journal),
        ] {
            self.bundle._directory.verify_child(name, identity)?;
        }
        Ok(())
    }

    fn rename(&mut self) -> Result<(), RenameFailure> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let result = fs::renameat_with(
            &self.bundle._parent,
            self.bundle.name.as_c_str(),
            &self.bundle._parent,
            self.retired.name(),
            RenameFlags::NOREPLACE,
        );
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let result = Err(rustix::io::Errno::NOSYS);
        match result {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::EXIST) => Err(RenameFailure::Collision(IoError::Io(
                ErrorKind::AlreadyExists,
            ))),
            Err(error) => Err(RenameFailure::Uncertain(IoError::from(
                std::io::Error::from(error),
            ))),
        }
    }

    fn sync_parent(&mut self) -> Result<(), IoError> {
        self.bundle._parent.sync()
    }

    fn cleanup(&mut self) -> Result<(), IoError> {
        self.bundle
            ._parent
            .verify_child(self.retired.name(), self.bundle.binding.bundle)?;
        for (name, identity) in [
            (JOURNAL_ENTRY, self.bundle.binding.journal),
            (INTENT_ENTRY, self.bundle.binding.intent),
            (BINDING_ENTRY, self.bundle.binding.binding),
        ] {
            self.bundle._directory.verify_child(name, identity)?;
            fs::unlinkat(&self.bundle._directory, name, AtFlags::empty())
                .map_err(std::io::Error::from)?;
        }
        self.bundle._directory.sync()?;
        self.bundle
            ._parent
            .verify_child(self.retired.name(), self.bundle.binding.bundle)?;
        fs::unlinkat(
            &self.bundle._parent,
            self.retired.name(),
            AtFlags::REMOVEDIR,
        )
        .map_err(std::io::Error::from)?;
        self.bundle._parent.sync()
    }
}

fn failure(
    phase: FinalizationPhase,
    outcome: RecoveryOutcome,
    cause: impl Into<PairError>,
) -> FinalizationError {
    FinalizationError {
        phase,
        outcome: Some(outcome),
        cause: cause.into(),
    }
}

fn retire_with(
    operations: &mut impl NamespaceOps,
    state: &mut PairState,
    permit: &mut TerminalPermit,
    outcome: RecoveryOutcome,
) -> Result<(), FinalizationError> {
    if *state != PairState::OutcomeRecorded || !permit.retire {
        return Err(failure(
            FinalizationPhase::State,
            outcome,
            PairError::InvalidState,
        ));
    }
    permit.retire = false;
    *state = PairState::RetirementBlocked;
    operations
        .verify_active()
        .map_err(|cause| failure(FinalizationPhase::Retirement, outcome, cause))?;
    match operations.rename() {
        Ok(()) => *state = PairState::RetirementUnconfirmed,
        Err(RenameFailure::Collision(cause)) => {
            return Err(failure(FinalizationPhase::Retirement, outcome, cause));
        }
        Err(RenameFailure::Uncertain(cause)) => {
            *state = PairState::RetirementUncertain;
            return Err(failure(FinalizationPhase::Retirement, outcome, cause));
        }
    }
    operations
        .sync_parent()
        .map_err(|cause| failure(FinalizationPhase::RetirementBarrier, outcome, cause))?;
    *state = PairState::Retired;
    Ok(())
}

pub(super) fn retire(
    bundle: &ExistingBundle,
    state: &mut PairState,
    permit: &mut TerminalPermit,
    outcome: RecoveryOutcome,
) -> Result<(), FinalizationError> {
    let mut operations = Actual {
        bundle,
        retired: RetirementName::new(bundle),
    };
    retire_with(&mut operations, state, permit, outcome)
}

fn confirm_barrier(
    state: &mut PairState,
    outcome: RecoveryOutcome,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    sync_parent: impl FnOnce() -> Result<(), IoError>,
) -> Result<(), FinalizationError> {
    if *state != PairState::RetirementUnconfirmed {
        return Err(failure(
            FinalizationPhase::State,
            outcome,
            PairError::InvalidState,
        ));
    }
    check(budget, cancelled)
        .map_err(|cause| failure(FinalizationPhase::RetirementBarrier, outcome, cause))?;
    sync_parent().map_err(|cause| failure(FinalizationPhase::RetirementBarrier, outcome, cause))?;
    *state = PairState::Retired;
    Ok(())
}

pub(super) fn confirm(
    bundle: &ExistingBundle,
    state: &mut PairState,
    outcome: RecoveryOutcome,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FinalizationError> {
    // Explicit new action retries ONLY this held barrier; no name construction,
    // rename, native recovery, SQL or cancellation callback follows success.
    confirm_barrier(state, outcome, budget, cancelled, || bundle._parent.sync())
}

fn cleanup_with(
    operations: &mut impl NamespaceOps,
    state: &mut PairState,
    permit: &mut TerminalPermit,
    outcome: RecoveryOutcome,
) -> Result<(), FinalizationError> {
    if *state != PairState::Retired || !permit.cleanup {
        return Err(failure(
            FinalizationPhase::State,
            outcome,
            PairError::InvalidState,
        ));
    }
    permit.cleanup = false;
    *state = PairState::RetiredGarbageUncertain;
    operations
        .cleanup()
        .map_err(|cause| failure(FinalizationPhase::GarbageCleanup, outcome, cause))?;
    *state = PairState::RetiredClean;
    Ok(())
}

pub(super) fn cleanup(
    bundle: &ExistingBundle,
    state: &mut PairState,
    permit: &mut TerminalPermit,
    outcome: RecoveryOutcome,
) -> Result<(), FinalizationError> {
    let mut operations = Actual {
        bundle,
        retired: RetirementName::new(bundle),
    };
    cleanup_with(&mut operations, state, permit, outcome)
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
