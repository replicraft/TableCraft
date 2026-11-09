//! Actor outcomes do not resolve the header; only the checked AFTER read does.
use super::*;
use crate::workspace::editing::field_save::{FieldOperation, FieldReadWitness};
use crate::workspace::editing::{cleanup_notice, save_outcome};
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveReport};
use tablecraft_ui::{Database, FieldAdditionSaveCommand};

impl Desktop {
    pub(in crate::workspace) fn cancel_current_field_addition_operation(&mut self) -> bool {
        let Some(host) = &self.editing.field_addition else {
            return false;
        };
        if let Some(identity) = host.identity {
            self.state.cancel_field_addition_operation(identity);
            self.field_addition_save_command(identity, FieldAdditionSaveCommand::Cancel);
        } else {
            let id = host.request_id;
            self.cancel_field_addition_review(id);
        }
        true
    }

    pub(in crate::workspace) fn note_field_addition_cancellation(&mut self, save_id: u64) {
        if let Some(host) = self.editing.field_addition.as_mut().filter(|host| {
            host.identity
                .is_some_and(|identity| identity.save_id == save_id)
        }) {
            host.cancelled = true;
        }
    }

    pub(in crate::workspace) fn field_addition_save_report(&mut self, report: SaveReport) {
        let Some(host) = &mut self.editing.field_addition else {
            return;
        };
        let Some(identity) = host
            .identity
            .filter(|identity| identity.save_id == report.id)
        else {
            return;
        };
        if !self
            .state
            .matches_field_addition_save(&host.request, identity)
        {
            return;
        }
        host.terminal = Some(report.outcome);
        self.state.set_field_addition_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary_success = !host.cancelled
            && report.operation == SaveOperation::Apply
            && match report.outcome {
                SaveOutcome::Saved { receipt } => {
                    FieldOperation::Addition
                        .facts(report.outcome)
                        .is_some_and(|facts| facts.replacement)
                        && receipt.completion == SaveCompletion::Applied
                }
                _ => false,
            };
        if ordinary_success {
            self.acknowledge_field_addition(identity);
        }
    }

    pub(in crate::workspace) fn field_addition_save_command(
        &mut self,
        identity: FieldAdditionSaveIdentity,
        command: FieldAdditionSaveCommand,
    ) {
        let Some(host) = &self.editing.field_addition else {
            return;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_field_addition_save(&host.request, identity)
        {
            return;
        }
        match command {
            FieldAdditionSaveCommand::Cancel => {
                let reopening = host.reopening;
                let request_id = host.request_id;
                self.editing.field_addition.as_mut().unwrap().cancelled = true;
                if reopening {
                    // Worker cancellation revokes reply publication immediately.
                    // Preserve the acknowledged AFTER owner and allow its exact
                    // witness to be retried or explicitly returned to the draft.
                    self.worker.cancel();
                    self.field_addition_reopen_failed(
                        request_id,
                        self.state.localizer.text("shell-operation-cancelled"),
                    );
                    self.state.busy = false;
                    self.pending = PendingOperation::Read;
                } else {
                    self.editing.worker.cancel(identity.save_id);
                }
            }
            FieldAdditionSaveCommand::Recover | FieldAdditionSaveCommand::Release => {
                let recover = command == FieldAdditionSaveCommand::Recover;
                if !self
                    .state
                    .can_resolve_field_addition_save(identity, recover)
                {
                    return;
                }
                let result = if recover {
                    self.editing.worker.recover(identity.save_id)
                } else {
                    self.editing.worker.release(identity.save_id)
                };
                match result {
                    Ok(next_id) => {
                        let host = self.editing.field_addition.as_mut().unwrap();
                        if let Some(next) = self.state.begin_field_addition_resolution(
                            &host.request,
                            identity,
                            next_id,
                            recover,
                        ) {
                            host.identity = Some(next);
                            host.terminal = None;
                            if let Some(save) = &mut self.editing.save {
                                save.id = next_id;
                                save.report = None;
                            }
                        }
                    }
                    Err(error) => self.state.error = Some(error.to_string()),
                }
            }
            FieldAdditionSaveCommand::Acknowledge => self.acknowledge_field_addition(identity),
            FieldAdditionSaveCommand::RetryReopen => {
                if self.state.can_retry_field_addition_reopen(identity) {
                    self.reopen_field_addition(identity);
                }
            }
            FieldAdditionSaveCommand::ReturnToDraft => {
                self.return_to_field_addition_draft(identity);
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_uncertain_field_addition(
        &mut self,
        save_id: u64,
    ) -> bool {
        let Some(host) = &self.editing.field_addition else {
            return false;
        };
        let Some(identity) = host.identity.filter(|identity| identity.save_id == save_id) else {
            return false;
        };
        if !self.state.can_acknowledge_field_addition_save(identity)
            || !self
                .state
                .matches_field_addition_save(&host.request, identity)
            || !self.editing.worker.acknowledge(save_id)
        {
            return false;
        }
        self.state
            .acknowledge_uncertain_field_addition_save(&host.request, identity);
        self.editing.field_addition = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_field_addition(
        &mut self,
        identity: FieldAdditionSaveIdentity,
    ) {
        if self.acknowledge_uncertain_save(identity.save_id) {
            return;
        }

        if self.acknowledge_field_addition_actor(identity) {
            self.reopen_field_addition(identity);
        }
    }

    pub(in crate::workspace) fn acknowledge_field_addition_actor(
        &mut self,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.field_addition else {
            return false;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_field_addition_save(&host.request, identity)
            || !self.state.can_acknowledge_field_addition_save(identity)
        {
            return false;
        }
        let Some(report) = self.field_actor_report(FieldOperation::Addition, identity.save_id)
        else {
            return false;
        };
        let Some(facts) = FieldOperation::Addition.facts(report.outcome) else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return false;
        };
        let replacement = facts.replacement;
        let completed = facts.completed;
        if replacement && host.witness.is_none() {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return false;
        }
        if self
            .acknowledge_field_actor(FieldOperation::Addition, identity.save_id)
            .is_none()
        {
            return false;
        }
        if replacement {
            self.editing.field_addition.as_mut().unwrap().cleanup =
                completed.and_then(cleanup_notice);
            true
        } else {
            let host = self.editing.field_addition.as_ref().unwrap();
            self.state
                .release_field_addition_save(&host.request, identity);
            self.editing.field_addition = None;
            self.restore_field_addition_close_after_refusal();
            false
        }
    }

    pub(in crate::workspace) fn reopen_field_addition(
        &mut self,
        identity: FieldAdditionSaveIdentity,
    ) {
        let Some(host) = &mut self.editing.field_addition else {
            return;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || !host.terminal.is_some_and(|outcome| {
                FieldOperation::Addition
                    .facts(outcome)
                    .is_some_and(|facts| facts.replacement)
            })
        {
            return;
        }
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        if !self.state.begin_field_addition_reopen(identity) {
            return;
        }
        host.reopening = true;
        let request_id = host.request_id;
        // Generic submit intentionally refuses while this exact schema owner
        // remains retained. Issue only its accepted full-image witness here,
        // after native ACK, just as the query-definition owner does.
        let Some(generation) = self.issue_field_witness_read(FieldReadWitness::Addition(witness))
        else {
            self.read_request_rejected();
            let message = self
                .state
                .localizer
                .text("objects-read-identities-exhausted");
            self.field_addition_reopen_failed(request_id, message);
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::FieldAdditionReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn field_addition_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_addition
            .as_mut()
            .filter(|host| host.request_id == request_id && host.reopening)
        else {
            return;
        };
        host.reopening = false;
        if let Some(identity) = host.identity {
            self.state.fail_field_addition_reopen(identity, message);
        }
    }

    /// Only the matching runtime full-content witness response reaches here.
    pub(in crate::workspace) fn finish_field_addition_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.field_addition else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        if host.request_id != request_id
            || !host.reopening
            || !host.terminal.is_some_and(|outcome| {
                FieldOperation::Addition
                    .facts(outcome)
                    .is_some_and(|facts| facts.replacement)
            })
        {
            return;
        }
        let revision = database.document_revision;
        if !self.state.resolve_field_addition_after_reopen(
            request_id,
            &host.request,
            identity.review_id,
            identity.save_id,
            database,
        ) {
            self.field_addition_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let host = self.editing.field_addition.as_mut().unwrap();
        self.state
            .clear_field_addition_save(&host.request, identity);
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
        self.editing.field_addition = None;
        self.resume_after_field_schema = true;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }
}
