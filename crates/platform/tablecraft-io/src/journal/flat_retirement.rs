//! Closed terminal lifecycle for retained flat filesystem journal artifacts.
//! The platform reserves all work before native effects. These transitions never
//! consult a shared budget, cancellation callback, pathname or native edit plan.
use super::RecoveryOutcome;
use super::filesystem_contracts::{FinalizationError, FinalizationPhase, PairError, PairState};
use crate::IoError;
use std::io::ErrorKind;

pub(super) trait NamespaceOps {
    fn verify(&mut self) -> Result<(), PairError>;
    fn rename(&mut self) -> Result<(), PairError>;
    fn flush(&mut self) -> Result<(), PairError>;
}

pub(super) struct TerminalPermit {
    retirement: bool,
    barrier_retry: bool,
}

fn failure(
    known: Option<RecoveryOutcome>,
    phase: FinalizationPhase,
    cause: PairError,
) -> FinalizationError {
    FinalizationError {
        phase,
        outcome: known,
        cause,
    }
}

impl TerminalPermit {
    /// Construction follows the platform's acknowledged terminal-work reserve.
    pub(super) const fn reserved(retired: bool) -> Self {
        Self {
            retirement: !retired,
            barrier_retry: true,
        }
    }

    pub(super) fn retire(
        &mut self,
        state: &mut PairState,
        known: &mut Option<RecoveryOutcome>,
        outcome: RecoveryOutcome,
        ops: &mut impl NamespaceOps,
    ) -> Result<(), FinalizationError> {
        if known.is_none() && *state == PairState::RecoveryRequired {
            *known = Some(outcome);
            *state = PairState::OutcomeRecorded;
        }
        if *known != Some(outcome) || *state != PairState::OutcomeRecorded || !self.retirement {
            return Err(failure(
                *known,
                FinalizationPhase::State,
                PairError::InvalidState,
            ));
        }
        self.retirement = false;
        if let Err(cause) = ops.verify() {
            *state = PairState::RetirementBlocked;
            return Err(failure(*known, FinalizationPhase::Retirement, cause));
        }
        *state = PairState::RetirementUncertain;
        if let Err(cause) = ops.rename() {
            if matches!(cause, PairError::Io(IoError::Io(ErrorKind::AlreadyExists))) {
                *state = PairState::RetirementBlocked;
            }
            return Err(failure(*known, FinalizationPhase::Retirement, cause));
        }
        Self::flush(state, *known, ops)
    }

    pub(super) fn confirm(
        &mut self,
        state: &mut PairState,
        known: Option<RecoveryOutcome>,
        outcome: RecoveryOutcome,
        ops: &mut impl NamespaceOps,
    ) -> Result<(), FinalizationError> {
        if *state != PairState::RetirementUnconfirmed
            || known != Some(outcome)
            || !self.barrier_retry
        {
            return Err(failure(
                known,
                FinalizationPhase::State,
                PairError::InvalidState,
            ));
        }
        self.barrier_retry = false;
        Self::flush(state, known, ops)
    }

    pub(super) fn acknowledge(
        &mut self,
        state: &mut PairState,
        known: &mut Option<RecoveryOutcome>,
        outcome: RecoveryOutcome,
        ops: &mut impl NamespaceOps,
    ) -> Result<(), FinalizationError> {
        if *state != PairState::RetiredVerificationRequired
            || known.is_some()
            || !self.barrier_retry
        {
            return Err(failure(
                *known,
                FinalizationPhase::State,
                PairError::InvalidState,
            ));
        }
        self.retirement = false;
        *known = Some(outcome);
        if let Err(cause) = ops.verify() {
            *state = PairState::RetirementBlocked;
            return Err(failure(*known, FinalizationPhase::Retirement, cause));
        }
        Self::flush(state, *known, ops)
    }

    fn flush(
        state: &mut PairState,
        known: Option<RecoveryOutcome>,
        ops: &mut impl NamespaceOps,
    ) -> Result<(), FinalizationError> {
        *state = PairState::RetirementUnconfirmed;
        ops.flush()
            .map_err(|cause| failure(known, FinalizationPhase::RetirementBarrier, cause))?;
        *state = PairState::Retired;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
