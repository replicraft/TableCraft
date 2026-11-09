//! Actual actor facts precede ACK, full checked read and atomic UI publication.
use super::TableDropHost;
use crate::workspace::editing::{
    cleanup_notice,
    field_save::{FieldOperation, FieldReadWitness},
    save_outcome,
};
use crate::workspace::{Desktop, PendingOperation};
use std::sync::Arc;
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveOutcome, SaveReport};
use tablecraft_ui::{Database, TableDropSaveCommand, TableDropSaveIdentity};

fn replacement_known(host: &TableDropHost) -> bool {
    host.save.terminal.is_some_and(|outcome| {
        FieldOperation::TableDrop
            .facts(outcome)
            .is_some_and(|facts| facts.replacement)
    })
}

impl Desktop {
    pub(in crate::workspace) fn table_drop_native_identity(
        &self,
        save_id: u64,
    ) -> Option<TableDropSaveIdentity> {
        self.editing
            .table_drop
            .as_ref()?
            .save
            .identity
            .filter(|identity| identity.save_id == save_id)
    }

    pub(in crate::workspace) fn cancel_current_table_drop_operation(&mut self) -> bool {
        let Some(host) = &self.editing.table_drop else {
            return false;
        };
        if let Some(identity) = host.save.identity {
            self.table_drop_save_command(identity, TableDropSaveCommand::Cancel);
        } else {
            self.cancel_table_drop_review(host.save.request_id);
        }
        true
    }

    pub(in crate::workspace) fn note_table_drop_cancellation(&mut self, save_id: u64) {
        if let Some(host) = self.editing.table_drop.as_mut().filter(|host| {
            host.save
                .identity
                .is_some_and(|identity| identity.save_id == save_id)
        }) {
            host.save.cancelled = true;
        }
    }

    pub(in crate::workspace) fn table_drop_save_report(&mut self, report: SaveReport) {
        if self.field_actor_report(FieldOperation::TableDrop, report.id) != Some(report) {
            return;
        }
        let Some(host) = &mut self.editing.table_drop else {
            return;
        };
        let Some(identity) = host
            .save
            .identity
            .filter(|identity| identity.save_id == report.id)
        else {
            return;
        };
        if !self
            .state
            .matches_table_drop_save(&host.save.request, identity)
        {
            return;
        }
        host.save.terminal = Some(report.outcome);
        self.state.set_table_drop_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary_success = !host.save.cancelled
            && report.operation == SaveOperation::Apply
            && matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.completion == SaveCompletion::Applied
                    && FieldOperation::TableDrop.facts(report.outcome).is_some_and(|facts| facts.replacement));
        if ordinary_success {
            self.acknowledge_table_drop(identity);
        }
    }

    pub(in crate::workspace) fn table_drop_save_command(
        &mut self,
        identity: TableDropSaveIdentity,
        command: TableDropSaveCommand,
    ) {
        let Some(host) = &self.editing.table_drop else {
            return;
        };
        if host.save.identity != Some(identity)
            || !self
                .state
                .matches_table_drop_save(&host.save.request, identity)
        {
            return;
        }
        match command {
            TableDropSaveCommand::Cancel => {
                if !self.state.cancel_table_drop_operation(identity) {
                    return;
                }
                let reopening = host.save.reopening;
                let request_id = host.save.request_id;
                if let Some(host) = self.editing.table_drop.as_mut() {
                    host.save.cancelled = true;
                }
                if reopening {
                    let current = matches!(self.pending, PendingOperation::TableDropReopen { request_id: pending } if pending == request_id);
                    if current {
                        self.worker.cancel();
                    }
                    self.table_drop_reopen_failed(
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
            TableDropSaveCommand::Recover | TableDropSaveCommand::Release => {
                let recover = command == TableDropSaveCommand::Recover;
                if self
                    .field_actor_report(FieldOperation::TableDrop, identity.save_id)
                    .is_none()
                {
                    return;
                }
                if !self.state.can_resolve_table_drop_save(identity, recover) {
                    return;
                }
                let result = if recover {
                    self.editing.worker.recover(identity.save_id)
                } else {
                    self.editing.worker.release(identity.save_id)
                };
                match result {
                    Ok(next_id) => {
                        let Some(next) = self
                            .state
                            .bind_table_drop_resolution(identity, recover, next_id)
                        else {
                            self.editing.worker.cancel(next_id);
                            self.state.mark_native_save_uncertain();
                            return;
                        };
                        let Some(host) = self.editing.table_drop.as_mut() else {
                            return;
                        };
                        host.save.identity = Some(next);
                        host.save.terminal = None;
                        let Some(save) = self.editing.save.as_mut() else {
                            return;
                        };
                        save.id = next_id;
                        save.report = None;
                    }
                    Err(error) => self.state.error = Some(error.to_string()),
                }
            }
            TableDropSaveCommand::Acknowledge => self.acknowledge_table_drop(identity),
            TableDropSaveCommand::RetryReopen => {
                if self.state.can_retry_table_drop_reopen(identity) {
                    self.reopen_table_drop(identity);
                }
            }
            TableDropSaveCommand::ReturnToCatalog => {
                self.return_to_table_drop_catalog(identity);
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_uncertain_table_drop(&mut self, save_id: u64) -> bool {
        let Some(host) = &self.editing.table_drop else {
            return false;
        };
        let Some(identity) = host
            .save
            .identity
            .filter(|identity| identity.save_id == save_id)
        else {
            return false;
        };
        if !self
            .state
            .matches_table_drop_save(&host.save.request, identity)
            || !self.state.can_acknowledge_table_drop_save(identity)
            || !self.editing.worker.acknowledge(save_id)
        {
            return false;
        }

        self.state.acknowledge_uncertain_table_drop_save(identity);
        self.editing.table_drop = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_table_drop(&mut self, identity: TableDropSaveIdentity) {
        if self.acknowledge_uncertain_save(identity.save_id) {
            return;
        }
        if self.acknowledge_table_drop_actor(identity) {
            self.reopen_table_drop(identity);
        }
    }

    pub(in crate::workspace) fn acknowledge_table_drop_actor(
        &mut self,
        identity: TableDropSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.table_drop else {
            return false;
        };
        if host.save.identity != Some(identity)
            || !self
                .state
                .matches_table_drop_save(&host.save.request, identity)
            || !self.state.can_acknowledge_table_drop_save(identity)
        {
            return false;
        }
        let Some(report) = self.field_actor_report(FieldOperation::TableDrop, identity.save_id)
        else {
            return false;
        };
        let Some(facts) = FieldOperation::TableDrop.facts(report.outcome) else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return false;
        };
        if facts.replacement && host.save.witness.is_none() {
            return false;
        }
        if self
            .acknowledge_field_actor(FieldOperation::TableDrop, identity.save_id)
            .is_none()
        {
            return false;
        }
        if facts.replacement {
            let Some(host) = self.editing.table_drop.as_mut() else {
                return false;
            };
            host.save.cleanup = facts.completed.and_then(cleanup_notice);
            true
        } else {
            self.state.release_table_drop_save(identity);
            self.editing.table_drop = None;
            self.pending = PendingOperation::Read;

            false
        }
    }

    pub(in crate::workspace) fn reopen_table_drop(&mut self, identity: TableDropSaveIdentity) {
        let Some(host) = &self.editing.table_drop else {
            return;
        };
        if host.save.identity != Some(identity)
            || host.save.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !replacement_known(host)
            || !self.state.begin_table_drop_reopen(identity)
        {
            return;
        }
        // NO_CLONE: DROP host retains complete-image witness for retry after failed AFTER read
        // while the explicit guarded read command independently owns a shared reference.
        let Some(witness) = host.save.witness.as_ref().map(Arc::clone) else {
            return;
        };
        let request_id = host.save.request_id;
        if let Some(host) = self.editing.table_drop.as_mut() {
            host.save.reopening = true;
        }
        let Some(generation) = self.issue_field_witness_read(FieldReadWitness::TableDrop(witness))
        else {
            self.read_request_rejected();
            let message = self
                .state
                .localizer
                .text("objects-read-identities-exhausted");
            self.table_drop_reopen_failed(request_id, message);
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::TableDropReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn table_drop_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .table_drop
            .as_mut()
            .filter(|host| host.save.request_id == request_id && host.save.reopening)
        else {
            return;
        };
        let Some(identity) = host.save.identity else {
            return;
        };
        host.save.reopening = false;
        self.state.fail_table_drop_reopen(identity, message);
    }

    pub(in crate::workspace) fn finish_table_drop_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.table_drop else {
            return;
        };
        let Some(identity) = host.save.identity else {
            return;
        };
        if host.save.request_id != request_id || !host.save.reopening || !replacement_known(host) {
            return;
        }
        let revision = database.document_revision;
        if !self
            .state
            .resolve_table_drop_after_reopen(&host.save.request, identity, database)
        {
            self.table_drop_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        // The operation-specific UI resolver atomically cleared only DROP.
        let Some(host) = self.editing.table_drop.as_mut() else {
            return;
        };
        let cleanup = host.save.cleanup.take();
        let filters = std::mem::take(&mut host.save.filters);
        self.finish_field_graph_reopen(
            FieldOperation::TableDrop,
            revision,
            table_creation_available,
            table_design_available,
            cleanup,
            filters,
        );
    }

    pub(in crate::workspace) fn return_to_table_drop_catalog(
        &mut self,
        identity: TableDropSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.table_drop else {
            return false;
        };
        if host.save.identity != Some(identity)
            || host.save.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !replacement_known(host)
            || !self.state.return_from_table_drop_reopen(identity)
        {
            return false;
        }
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        true
    }
}
