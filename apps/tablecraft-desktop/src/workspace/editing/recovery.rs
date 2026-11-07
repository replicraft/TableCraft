//! Bind explicit restart actions to the failed Open, never editable path text.
use super::{
    Desktop, EditSaveOutcome, SaveOutcome, SaveReport, cleanup_notice, outcome_details,
    save_outcome,
};
use crate::workspace::PendingOperation;
use std::path::PathBuf;
use tablecraft_runtime::Error;
use tablecraft_ui::{Action, Localizer, RecoveryOutcome, SaveCleanupNotice};

pub(in crate::workspace) struct RecoveryHost {
    attempt: u64,
    path: PathBuf,
    operation: Option<Operation>,
}

struct Operation {
    id: u64,
    report: Option<SaveReport>,
}

pub(in crate::workspace) struct RecoveryReopen {
    generation: u64,
    message: String,
    cleanup: Option<SaveCleanupNotice>,
}

pub(in crate::workspace) fn is_recovery_action(action: &Action) -> bool {
    matches!(
        action,
        Action::BeginRecovery { .. }
            | Action::CloseRecovery { .. }
            | Action::CancelRecovery { .. }
            | Action::ResolveRecovery { .. }
            | Action::AcknowledgeRecovery { .. }
    )
}

impl Desktop {
    pub(in crate::workspace) fn recovery_open_failed(
        &mut self,
        generation: u64,
        error: &Error,
    ) -> bool {
        if self
            .recovery_reopen
            .as_ref()
            .is_some_and(|notice| notice.generation == generation)
        {
            let notice = self.recovery_reopen.take().unwrap();
            self.state.error = Some(format!("{}\n{error}", notice.message));
            return true;
        }
        let PendingOperation::Open { request_id, path } = &self.pending else {
            return false;
        };
        if *request_id != generation || !matches!(error, Error::RetainedJournal) {
            return false;
        }
        self.state
            .show_recovery_prompt(generation, path.to_string_lossy().into_owned());
        self.recovery = Some(RecoveryHost {
            attempt: generation,
            path: path.clone(),
            operation: None,
        });
        self.state.error = None;
        true
    }

    pub(in crate::workspace) fn begin_reserved_recovery(&mut self, attempt_id: u64) {
        let Some(owner) = self
            .recovery
            .as_ref()
            .filter(|owner| owner.attempt == attempt_id && owner.operation.is_none())
        else {
            return;
        };
        if self.editing.has_save() || !self.state.can_begin_recovery(attempt_id) {
            return;
        }
        match self
            .editing
            .worker
            .start_reserved_recovery(owner.path.clone())
        {
            Ok(id) => {
                self.state.begin_recovery(attempt_id, id);
                self.recovery.as_mut().unwrap().operation = Some(Operation { id, report: None });
            }
            Err(error) => self
                .state
                .fail_recovery_start(attempt_id, error.to_string()),
        }
    }

    pub(in crate::workspace) fn close_reserved_recovery(&mut self, attempt_id: u64) {
        let allowed = self
            .recovery
            .as_ref()
            .is_some_and(|owner| owner.attempt == attempt_id && owner.operation.is_none());
        if allowed && self.state.close_recovery(attempt_id) {
            self.recovery = None;
        }
    }

    pub(in crate::workspace) fn poll_recovery(&mut self) {
        let Some(report) = self.editing.worker.poll() else {
            return;
        };
        let Some(operation) = self
            .recovery
            .as_mut()
            .and_then(|owner| owner.operation.as_mut())
        else {
            return;
        };
        if operation.id != report.id || operation.report == Some(report) {
            return;
        }
        operation.report = Some(report);
        let outcome = recovery_outcome(&self.state.localizer, report.outcome);
        self.state.set_recovery_outcome(report.id, outcome);
    }

    pub(in crate::workspace) fn cancel_active_recovery(&self) {
        if let Some(operation) = self
            .recovery
            .as_ref()
            .and_then(|owner| owner.operation.as_ref())
        {
            self.editing.worker.cancel(operation.id);
        }
    }

    pub(in crate::workspace) fn cancel_reserved_recovery(&self, operation_id: u64) {
        if self
            .recovery
            .as_ref()
            .and_then(|owner| owner.operation.as_ref())
            .is_some_and(|operation| operation.id == operation_id)
        {
            self.editing.worker.cancel(operation_id);
        }
    }

    pub(in crate::workspace) fn resolve_reserved_recovery(
        &mut self,
        operation_id: u64,
        recover: bool,
    ) {
        let Some(operation) = self
            .recovery
            .as_ref()
            .and_then(|owner| owner.operation.as_ref())
        else {
            return;
        };
        let Some(report) = operation.report.filter(|report| report.id == operation_id) else {
            return;
        };
        if operation.id != operation_id
            || !self.state.can_resolve_recovery(operation_id, recover)
            || self.state.recovery_attempt_id() != self.recovery.as_ref().map(|owner| owner.attempt)
        {
            return;
        }
        let SaveOutcome::Failed(failure) = report.outcome else {
            return;
        };
        if !(if recover {
            failure.can_recover
        } else {
            failure.can_release
        }) {
            return;
        }
        let result = if recover {
            self.editing.worker.recover(operation_id)
        } else {
            self.editing.worker.release(operation_id)
        };
        match result {
            Ok(next) => {
                self.state.begin_recovery_resolution(operation_id, next);
                self.recovery.as_mut().unwrap().operation = Some(Operation {
                    id: next,
                    report: None,
                });
            }
            Err(error) => self
                .state
                .fail_recovery_resolution(operation_id, error.to_string()),
        }
    }

    pub(in crate::workspace) fn acknowledge_reserved_recovery(&mut self, operation_id: u64) {
        let Some(operation) = self
            .recovery
            .as_ref()
            .and_then(|owner| owner.operation.as_ref())
        else {
            return;
        };
        let Some(report) = operation.report.filter(|report| report.id == operation_id) else {
            return;
        };
        if operation.id != operation_id
            || !self.state.can_acknowledge_recovery(operation_id)
            || self.state.recovery_attempt_id() != self.recovery.as_ref().map(|owner| owner.attempt)
        {
            return;
        }
        if !self.editing.worker.acknowledge(operation_id) {
            return;
        }
        self.state.acknowledge_recovery(operation_id);
        let owner = self.recovery.take().unwrap();
        if let SaveOutcome::Saved { receipt } = report.outcome {
            let message =
                outcome_details(&self.state.localizer, Some(receipt.known), Some(receipt));
            self.select_file(owner.path);
            self.open();
            self.recovery_reopen = Some(RecoveryReopen {
                generation: self.generation,
                message,
                cleanup: cleanup_notice(receipt),
            });
        }
    }

    pub(in crate::workspace) fn recovery_opened(&mut self, generation: u64, revision: u64) {
        if self
            .recovery_reopen
            .as_ref()
            .is_some_and(|notice| notice.generation == generation)
        {
            let notice = self.recovery_reopen.take().unwrap();
            self.state.set_save_cleanup(revision, notice.cleanup);
        }
    }
}

fn recovery_outcome(localizer: &Localizer, outcome: SaveOutcome) -> RecoveryOutcome {
    if let SaveOutcome::Saved { receipt } = outcome {
        return RecoveryOutcome::Finished(outcome_details(
            localizer,
            Some(receipt.known),
            Some(receipt),
        ));
    }
    match save_outcome(localizer, outcome) {
        EditSaveOutcome::Retained {
            message,
            can_recover,
            can_release,
        } => RecoveryOutcome::Retained {
            message,
            can_recover,
            can_release,
        },
        EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Failed(message) => RecoveryOutcome::Finished(message),
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
