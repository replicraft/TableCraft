//! Actual actor facts precede ACK, full checked read and atomic UI publication.
use super::FieldDropHost;
use crate::workspace::editing::{
    cleanup_notice,
    field_save::{FieldOperation, FieldReadWitness},
    save_outcome,
};
use crate::workspace::{Desktop, PendingOperation};
use std::sync::Arc;
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveOutcome, SaveReport};
use tablecraft_ui::{Database, FieldDropSaveCommand, FieldDropSaveIdentity};

fn replacement_known(host: &FieldDropHost) -> bool {
    host.terminal.is_some_and(|outcome| {
        FieldOperation::Drop
            .facts(outcome)
            .is_some_and(|facts| facts.replacement)
    })
}

impl Desktop {
    pub(in crate::workspace) fn field_drop_native_identity(
        &self,
        save_id: u64,
    ) -> Option<FieldDropSaveIdentity> {
        self.editing
            .field_drop
            .as_ref()?
            .identity
            .filter(|identity| identity.save_id == save_id)
    }

    pub(in crate::workspace) fn cancel_current_field_drop_operation(&mut self) -> bool {
        let Some(host) = &self.editing.field_drop else {
            return false;
        };
        if let Some(identity) = host.identity {
            self.field_drop_save_command(identity, FieldDropSaveCommand::Cancel);
        } else {
            self.cancel_field_drop_review(host.request_id);
        }
        true
    }

    pub(in crate::workspace) fn note_field_drop_cancellation(&mut self, save_id: u64) {
        if let Some(host) = self.editing.field_drop.as_mut().filter(|host| {
            host.identity
                .is_some_and(|identity| identity.save_id == save_id)
        }) {
            host.cancelled = true;
        }
    }

    pub(in crate::workspace) fn field_drop_save_report(&mut self, report: SaveReport) {
        if self.field_actor_report(FieldOperation::Drop, report.id) != Some(report) {
            return;
        }
        let Some(host) = &mut self.editing.field_drop else {
            return;
        };
        let Some(identity) = host
            .identity
            .filter(|identity| identity.save_id == report.id)
        else {
            return;
        };
        if !self.state.matches_field_drop_save(&host.request, identity) {
            return;
        }
        host.terminal = Some(report.outcome);
        self.state.set_field_drop_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary_success = !host.cancelled
            && report.operation == SaveOperation::Apply
            && matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.completion == SaveCompletion::Applied
                    && FieldOperation::Drop.facts(report.outcome).is_some_and(|facts| facts.replacement));
        if ordinary_success {
            self.acknowledge_field_drop(identity);
        }
    }

    pub(in crate::workspace) fn field_drop_save_command(
        &mut self,
        identity: FieldDropSaveIdentity,
        command: FieldDropSaveCommand,
    ) {
        let Some(host) = &self.editing.field_drop else {
            return;
        };
        if host.identity != Some(identity)
            || !self.state.matches_field_drop_save(&host.request, identity)
        {
            return;
        }
        match command {
            FieldDropSaveCommand::Cancel => {
                if !self.state.cancel_field_drop_operation(identity) {
                    return;
                }
                let reopening = host.reopening;
                let request_id = host.request_id;
                self.editing.field_drop.as_mut().unwrap().cancelled = true;
                if reopening {
                    let current = matches!(self.pending, PendingOperation::FieldDropReopen { request_id: pending } if pending == request_id);
                    if current {
                        self.worker.cancel();
                    }
                    self.field_drop_reopen_failed(
                        request_id,
                        self.state.localizer.text("shell-operation-cancelled"),
                    );
                    if current {
                        self.pending = PendingOperation::Read;
                        self.state.busy = false;
                    }
                } else {
                    self.editing.worker.cancel(identity.save_id);
                }
            }
            FieldDropSaveCommand::Recover | FieldDropSaveCommand::Release => {
                let recover = command == FieldDropSaveCommand::Recover;
                if self
                    .field_actor_report(FieldOperation::Drop, identity.save_id)
                    .is_none()
                {
                    return;
                }
                let host = self.editing.field_drop.as_ref().unwrap();
                let Some(prepared) =
                    self.state
                        .prepare_field_drop_resolution(&host.request, identity, recover)
                else {
                    return;
                };
                let result = if recover {
                    self.editing.worker.recover(identity.save_id)
                } else {
                    self.editing.worker.release(identity.save_id)
                };
                match result {
                    Ok(next_id) => {
                        let next = prepared.bind(next_id);
                        let host = self.editing.field_drop.as_mut().unwrap();
                        host.identity = Some(next);
                        host.terminal = None;
                        let save = self.editing.save.as_mut().unwrap();
                        save.id = next_id;
                        save.report = None;
                    }
                    Err(error) => self.state.error = Some(error.to_string()),
                }
            }
            FieldDropSaveCommand::Acknowledge => self.acknowledge_field_drop(identity),
            FieldDropSaveCommand::RetryReopen => {
                if self.state.can_retry_field_drop_reopen(identity) {
                    self.reopen_field_drop(identity);
                }
            }
            FieldDropSaveCommand::ReturnToDraft => {
                self.return_to_field_drop_draft(identity);
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_uncertain_field_drop(&mut self, save_id: u64) -> bool {
        let Some(host) = &self.editing.field_drop else {
            return false;
        };
        let Some(identity) = host.identity.filter(|identity| identity.save_id == save_id) else {
            return false;
        };
        if !self.state.matches_field_drop_save(&host.request, identity)
            || !self.state.can_acknowledge_field_drop_save(identity)
            || !self.editing.worker.acknowledge(save_id)
        {
            return false;
        }

        self.state
            .acknowledge_uncertain_field_drop_save(&host.request, identity);
        self.editing.field_drop = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_field_drop(&mut self, identity: FieldDropSaveIdentity) {
        if self.acknowledge_uncertain_save(identity.save_id) {
            return;
        }
        if self.acknowledge_field_drop_actor(identity) {
            self.reopen_field_drop(identity);
        }
    }

    pub(in crate::workspace) fn acknowledge_field_drop_actor(
        &mut self,
        identity: FieldDropSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.field_drop else {
            return false;
        };
        if host.identity != Some(identity)
            || !self.state.matches_field_drop_save(&host.request, identity)
            || !self.state.can_acknowledge_field_drop_save(identity)
        {
            return false;
        }
        let Some(report) = self.field_actor_report(FieldOperation::Drop, identity.save_id) else {
            return false;
        };
        let Some(facts) = FieldOperation::Drop.facts(report.outcome) else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return false;
        };
        if facts.replacement && host.witness.is_none() {
            return false;
        }
        if self
            .acknowledge_field_actor(FieldOperation::Drop, identity.save_id)
            .is_none()
        {
            return false;
        }
        if facts.replacement {
            self.editing.field_drop.as_mut().unwrap().cleanup =
                facts.completed.and_then(cleanup_notice);
            true
        } else {
            let host = self.editing.field_drop.as_ref().unwrap();
            self.state.release_field_drop_save(&host.request, identity);
            self.editing.field_drop = None;
            self.pending = PendingOperation::Read;
            self.restore_field_drop_close_after_refusal();
            false
        }
    }

    pub(in crate::workspace) fn reopen_field_drop(&mut self, identity: FieldDropSaveIdentity) {
        let Some(host) = &self.editing.field_drop else {
            return;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !replacement_known(host)
            || !self.state.begin_field_drop_reopen(identity)
        {
            return;
        }
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        let request_id = host.request_id;
        self.editing.field_drop.as_mut().unwrap().reopening = true;
        let Some(generation) = self.issue_field_witness_read(FieldReadWitness::Drop(witness))
        else {
            self.read_request_rejected();
            let message = self
                .state
                .localizer
                .text("objects-read-identities-exhausted");
            self.field_drop_reopen_failed(request_id, message);
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::FieldDropReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn field_drop_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_drop
            .as_mut()
            .filter(|host| host.request_id == request_id && host.reopening)
        else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        host.reopening = false;
        self.state.fail_field_drop_reopen(identity, message);
    }

    pub(in crate::workspace) fn finish_field_drop_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.field_drop else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        if host.request_id != request_id || !host.reopening || !replacement_known(host) {
            return;
        }
        let revision = database.document_revision;
        if !self
            .state
            .resolve_field_drop_after_reopen(&host.request, identity, database)
        {
            self.field_drop_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        // The operation-specific UI resolver atomically cleared only DROP.
        let host = self.editing.field_drop.as_mut().unwrap();
        let cleanup = host.cleanup.take();
        let filters = std::mem::take(&mut host.filters);
        self.finish_field_graph_reopen(
            FieldOperation::Drop,
            revision,
            table_creation_available,
            table_design_available,
            cleanup,
            filters,
        );
    }

    pub(in crate::workspace) fn return_to_field_drop_draft(
        &mut self,
        identity: FieldDropSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.field_drop else {
            return false;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !replacement_known(host)
            || !self.state.can_return_to_field_drop_draft(identity)
        {
            return false;
        }
        let cleanup = host.cleanup;
        if !self
            .state
            .return_field_drop_to_draft(&host.request, identity, cleanup)
        {
            return false;
        }
        if let Some(database) = &self.state.database {
            self.state
                .set_save_cleanup(database.document_revision, cleanup);
        }
        self.editing.field_drop = None;
        self.cancel_table_draft_action();
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        true
    }
}
