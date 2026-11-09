//! Real actor facts and accepted readonly witnesses compose with shared UI state.
use super::*;
use crate::workspace::editing::{cleanup_notice, field_save::FieldOperation, save_outcome};
use std::sync::Arc;
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveOutcome, SaveReport};
use tablecraft_ui::{Database, FieldRenameSaveCommand};

fn native_id(identity: FieldRenameSaveIdentity) -> Option<u64> {
    match identity {
        FieldRenameSaveIdentity::NativeSave { save_id, .. } => Some(save_id),
        FieldRenameSaveIdentity::UnchangedReview { .. } => None,
    }
}

fn witnessed_read_authorized(host: &FieldRenameHost) -> bool {
    match host.identity {
        Some(FieldRenameSaveIdentity::UnchangedReview { .. }) => {
            host.terminal.is_none()
                && host
                    .witness
                    .as_ref()
                    .is_some_and(|witness| !witness.changed())
        }
        Some(FieldRenameSaveIdentity::NativeSave { .. }) => host.terminal.is_some_and(|outcome| {
            FieldOperation::Rename
                .facts(outcome)
                .is_some_and(|facts| facts.replacement)
        }),
        None => false,
    }
}

impl Desktop {
    pub(in crate::workspace) fn field_rename_native_identity(
        &self,
        save_id: u64,
    ) -> Option<FieldRenameSaveIdentity> {
        self.editing
            .field_rename
            .as_ref()?
            .identity
            .filter(|identity| native_id(*identity) == Some(save_id))
    }

    pub(in crate::workspace) fn cancel_current_field_rename_operation(&mut self) -> bool {
        let Some(host) = &self.editing.field_rename else {
            return false;
        };
        if let Some(identity) = host.identity {
            self.field_rename_save_command(identity, FieldRenameSaveCommand::Cancel);
        } else {
            self.cancel_field_rename_review(host.request_id);
        }
        true
    }

    pub(in crate::workspace) fn note_field_rename_cancellation(&mut self, save_id: u64) {
        if let Some(host) = self
            .editing
            .field_rename
            .as_mut()
            .filter(|host| host.identity.and_then(native_id) == Some(save_id))
        {
            host.cancelled = true;
        }
    }

    pub(in crate::workspace) fn field_rename_save_report(&mut self, report: SaveReport) {
        let Some(host) = &mut self.editing.field_rename else {
            return;
        };
        let Some(identity) = host
            .identity
            .filter(|identity| native_id(*identity) == Some(report.id))
        else {
            return;
        };
        if !self
            .state
            .matches_field_rename_save(&host.request, identity)
        {
            return;
        }
        host.terminal = Some(report.outcome);
        self.state.set_field_rename_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary_success = !host.cancelled
            && report.operation == SaveOperation::Apply
            && matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.completion == SaveCompletion::Applied
                    && FieldOperation::Rename.facts(report.outcome).is_some_and(|facts| facts.replacement));
        if ordinary_success {
            self.acknowledge_field_rename(identity);
        }
    }

    pub(in crate::workspace) fn field_rename_save_command(
        &mut self,
        identity: FieldRenameSaveIdentity,
        command: FieldRenameSaveCommand,
    ) {
        let Some(host) = &self.editing.field_rename else {
            return;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_field_rename_save(&host.request, identity)
        {
            return;
        }
        match command {
            FieldRenameSaveCommand::Cancel => {
                if !self.state.cancel_field_rename_operation(identity) {
                    return;
                }
                let reopening = host.reopening;
                let request_id = host.request_id;
                self.editing.field_rename.as_mut().unwrap().cancelled = true;
                if reopening {
                    self.worker.cancel();
                    self.field_rename_reopen_failed(
                        request_id,
                        self.state.localizer.text("shell-operation-cancelled"),
                    );
                    self.pending = PendingOperation::Read;
                    self.state.busy = false;
                } else if let Some(save_id) = native_id(identity) {
                    self.editing.worker.cancel(save_id);
                }
            }
            FieldRenameSaveCommand::Recover | FieldRenameSaveCommand::Release => {
                let Some(save_id) = native_id(identity) else {
                    return;
                };
                let recover = command == FieldRenameSaveCommand::Recover;
                if self
                    .field_actor_report(FieldOperation::Rename, save_id)
                    .is_none()
                {
                    return;
                }
                let host = self.editing.field_rename.as_ref().unwrap();
                let Some(prepared) =
                    self.state
                        .prepare_field_rename_resolution(&host.request, identity, recover)
                else {
                    return;
                };
                let result = if recover {
                    self.editing.worker.recover(save_id)
                } else {
                    self.editing.worker.release(save_id)
                };
                match result {
                    Ok(next_id) => {
                        let next = prepared.bind(next_id);
                        let host = self.editing.field_rename.as_mut().unwrap();
                        host.identity = Some(next);
                        host.terminal = None;
                        let save = self.editing.save.as_mut().unwrap();
                        save.id = next_id;
                        save.report = None;
                    }
                    Err(error) => {
                        self.state.error = Some(error.to_string());
                    }
                }
            }
            FieldRenameSaveCommand::Acknowledge => self.acknowledge_field_rename(identity),
            FieldRenameSaveCommand::RetryReopen => {
                if self.state.can_retry_field_rename_reopen(identity) {
                    self.reopen_field_rename(identity);
                }
            }
            FieldRenameSaveCommand::ReturnToDraft => {
                self.return_to_field_rename_draft(identity);
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_field_rename(
        &mut self,
        identity: FieldRenameSaveIdentity,
    ) {
        if self.acknowledge_field_rename_actor(identity) {
            self.reopen_field_rename(identity);
        }
    }

    pub(in crate::workspace) fn acknowledge_field_rename_actor(
        &mut self,
        identity: FieldRenameSaveIdentity,
    ) -> bool {
        let Some(save_id) = native_id(identity) else {
            return false;
        };
        let Some(host) = &self.editing.field_rename else {
            return false;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_field_rename_save(&host.request, identity)
            || !self.state.can_acknowledge_field_rename_save(identity)
        {
            return false;
        }
        let Some(report) = self.field_actor_report(FieldOperation::Rename, save_id) else {
            return false;
        };
        let Some(facts) = FieldOperation::Rename.facts(report.outcome) else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return false;
        };
        if facts.replacement && host.witness.is_none() {
            return false;
        }
        if self
            .acknowledge_field_actor(FieldOperation::Rename, save_id)
            .is_none()
        {
            return false;
        }
        if facts.replacement {
            self.editing.field_rename.as_mut().unwrap().cleanup =
                facts.completed.and_then(cleanup_notice);
            true
        } else {
            let host = self.editing.field_rename.as_ref().unwrap();
            self.state
                .release_field_rename_save(&host.request, identity);
            self.editing.field_rename = None;
            self.restore_field_rename_close_after_refusal();
            false
        }
    }

    pub(in crate::workspace) fn reopen_field_rename(&mut self, identity: FieldRenameSaveIdentity) {
        let Some(host) = &self.editing.field_rename else {
            return;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !witnessed_read_authorized(host)
            || !self.state.begin_field_rename_reopen(identity)
        {
            return;
        }
        let witness = Arc::clone(host.witness.as_ref().unwrap());
        let request_id = host.request_id;
        self.editing.field_rename.as_mut().unwrap().reopening = true;
        self.generation = self.issue_field_witness_read(FieldReadWitness::Rename(witness));
        self.pending = PendingOperation::FieldRenameReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn field_rename_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_rename
            .as_mut()
            .filter(|host| host.request_id == request_id && host.reopening)
        else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        host.reopening = false;
        self.state.fail_field_rename_reopen(identity, message);
    }

    pub(in crate::workspace) fn finish_field_rename_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.field_rename else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        if host.request_id != request_id || !host.reopening || !witnessed_read_authorized(host) {
            return;
        }
        let revision = database.document_revision;
        if !self
            .state
            .resolve_field_rename_after_reopen(&host.request, identity, database)
        {
            self.field_rename_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let host = self.editing.field_rename.as_mut().unwrap();
        self.state.clear_field_rename_save(&host.request, identity);
        let cleanup = host.cleanup.take();
        let filters = std::mem::take(&mut host.filters);
        self.retain_filter_restore_definitions(filters);
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, cleanup);
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        self.editing.field_rename = None;
        self.resume_after_field_schema = true;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    pub(in crate::workspace) fn return_to_field_rename_draft(
        &mut self,
        identity: FieldRenameSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.field_rename else {
            return false;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !witnessed_read_authorized(host)
            || !self.state.can_return_to_field_rename_draft(identity)
        {
            return false;
        }
        let cleanup = host.cleanup;
        if !self
            .state
            .return_field_rename_to_draft(&host.request, identity, cleanup)
        {
            return false;
        }
        if let Some(database) = &self.state.database {
            self.state
                .set_save_cleanup(database.document_revision, cleanup);
        }
        self.editing.field_rename = None;
        self.cancel_table_draft_action();
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        true
    }
}
