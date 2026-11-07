//! Restart recovery presentation; the host retains actual paths and resources.
mod dialog;

use super::State;

pub enum RecoveryOutcome {
    /// A terminal report. It does not imply that a commit or rollback occurred.
    Finished(String),
    /// Actual retained-resource capabilities supplied by the runtime owner.
    Retained {
        message: String,
        can_recover: bool,
        can_release: bool,
    },
}

pub(super) struct Recovery {
    attempt: u64,
    path: String,
    phase: Phase,
    focus_cancel: bool,
    error: Option<String>,
}

pub(super) enum Phase {
    Prompt,
    Running {
        operation: u64,
        cancel_requested: bool,
    },
    Outcome {
        operation: u64,
        outcome: RecoveryOutcome,
    },
}

impl State {
    /// Show one explicit failed-open attempt; another attempt cannot replace it.
    pub fn show_recovery_prompt(&mut self, attempt_id: u64, path: String) {
        if attempt_id == 0 || self.recovery.is_some() {
            return;
        }
        self.recovery = Some(Recovery {
            attempt: attempt_id,
            path,
            phase: Phase::Prompt,
            focus_cancel: true,
            error: None,
        });
    }

    pub fn recovery_prompt_open(&self) -> bool {
        self.recovery.is_some()
    }

    pub fn recovery_pending(&self) -> bool {
        matches!(
            self.recovery.as_ref().map(|owner| &owner.phase),
            Some(Phase::Running { .. })
        )
    }

    pub fn recovery_attempt_id(&self) -> Option<u64> {
        self.recovery.as_ref().map(|owner| owner.attempt)
    }

    pub fn can_begin_recovery(&self, attempt_id: u64) -> bool {
        self.recovery.as_ref().is_some_and(|owner| {
            owner.attempt == attempt_id && matches!(owner.phase, Phase::Prompt)
        })
    }

    pub fn fail_recovery_start(&mut self, attempt_id: u64, message: String) {
        if self.can_begin_recovery(attempt_id) {
            self.recovery.as_mut().unwrap().error = Some(message);
        }
    }

    pub fn begin_recovery(&mut self, attempt_id: u64, operation_id: u64) -> bool {
        if operation_id == 0 || !self.can_begin_recovery(attempt_id) {
            return false;
        }
        let owner = self.recovery.as_mut().unwrap();
        owner.error = None;
        owner.phase = Phase::Running {
            operation: operation_id,
            cancel_requested: false,
        };
        true
    }

    pub fn can_resolve_recovery(&self, operation_id: u64, recover: bool) -> bool {
        matches!(
            self.recovery.as_ref().map(|owner| &owner.phase),
            Some(Phase::Outcome {
                operation,
                outcome: RecoveryOutcome::Retained { can_recover, can_release, .. },
            }) if *operation == operation_id && if recover { *can_recover } else { *can_release }
        )
    }

    pub fn fail_recovery_resolution(&mut self, operation_id: u64, message: String) {
        let Some(owner) = &mut self.recovery else {
            return;
        };
        if matches!(&owner.phase,
            Phase::Outcome { operation, outcome: RecoveryOutcome::Retained { .. } }
            if *operation == operation_id)
        {
            owner.error = Some(message);
        }
    }

    pub fn begin_recovery_resolution(&mut self, old_id: u64, next_id: u64) -> bool {
        let Some(owner) = &mut self.recovery else {
            return false;
        };
        if next_id == 0
            || next_id == old_id
            || !matches!(&owner.phase,
            Phase::Outcome { operation, outcome: RecoveryOutcome::Retained { can_recover, can_release, .. } }
            if *operation == old_id && (*can_recover || *can_release))
        {
            return false;
        }
        owner.phase = Phase::Running {
            operation: next_id,
            cancel_requested: false,
        };
        owner.error = None;
        true
    }

    pub fn set_recovery_outcome(&mut self, operation_id: u64, outcome: RecoveryOutcome) {
        let Some(owner) = &mut self.recovery else {
            return;
        };
        if matches!(owner.phase, Phase::Running { operation, .. } if operation == operation_id) {
            owner.phase = Phase::Outcome {
                operation: operation_id,
                outcome,
            };
        }
    }

    /// Dismiss only the untouched prompt. No retained resource is released here.
    pub fn close_recovery(&mut self, attempt_id: u64) -> bool {
        if !self.recovery.as_ref().is_some_and(|owner| {
            owner.attempt == attempt_id && matches!(owner.phase, Phase::Prompt)
        }) {
            return false;
        }
        self.recovery = None;
        true
    }

    pub fn can_acknowledge_recovery(&self, operation_id: u64) -> bool {
        self.recovery.as_ref().is_some_and(|owner| matches!(&owner.phase,
            Phase::Outcome { operation, outcome: RecoveryOutcome::Finished(_) } if *operation == operation_id))
    }

    pub fn acknowledge_recovery(&mut self, operation_id: u64) -> bool {
        if !self.can_acknowledge_recovery(operation_id) {
            return false;
        }
        self.recovery = None;
        true
    }
}

#[cfg(test)]
mod tests;
