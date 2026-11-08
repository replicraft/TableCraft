//! Actor outcomes do not resolve the header; only the checked AFTER read does.
use super::*;
use crate::workspace::editing::{cleanup_notice, save_outcome};
use tablecraft_runtime::{
    SaveCompletion, SaveKnownOutcome, SaveOperation, SaveReceipt, SaveReport, SaveSchemaChange,
};
use tablecraft_ui::{Database, NumberColumnSaveCommand};

fn replacement_known(known: SaveKnownOutcome) -> bool {
    matches!(
        known,
        SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
    )
}

fn column_receipt(receipt: SaveReceipt) -> bool {
    replacement_known(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
        && receipt.query_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

/// Current original facts override an earlier receipt. Released AFTER may lack
/// a finalization receipt; no completed receipt is manufactured in that case.
fn reopen_facts(outcome: SaveOutcome) -> Result<(bool, Option<SaveReceipt>), ()> {
    match outcome {
        SaveOutcome::Saved { receipt } => {
            if replacement_known(receipt.known) && !column_receipt(receipt) {
                return Err(());
            }
            let replacement = column_receipt(receipt);
            Ok((replacement, replacement.then_some(receipt)))
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => {
            let completed = last_completed.filter(|receipt| column_receipt(*receipt));
            Ok((
                current_outcome.map_or(completed.is_some(), replacement_known),
                completed,
            ))
        }
        SaveOutcome::Failed(_) => Ok((false, None)),
    }
}

impl Desktop {
    pub(in crate::workspace) fn cancel_current_number_column_operation(&mut self) -> bool {
        let Some(host) = &self.editing.number_column else {
            return false;
        };
        if let Some(identity) = host.identity {
            self.state.cancel_number_column_operation(identity);
            self.number_column_save_command(identity, NumberColumnSaveCommand::Cancel);
        } else {
            let id = host.request_id;
            self.cancel_number_column_review(id);
        }
        true
    }

    pub(in crate::workspace) fn note_number_column_cancellation(&mut self, save_id: u64) {
        if let Some(host) = self.editing.number_column.as_mut().filter(|host| {
            host.identity
                .is_some_and(|identity| identity.save_id == save_id)
        }) {
            host.cancelled = true;
        }
    }

    pub(in crate::workspace) fn number_column_save_report(&mut self, report: SaveReport) {
        let Some(host) = &mut self.editing.number_column else {
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
            .matches_number_column_save(&host.request, identity)
        {
            return;
        }
        host.terminal = Some(report.outcome);
        self.state.set_number_column_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary_success = !host.cancelled
            && report.operation == SaveOperation::Apply
            && matches!(report.outcome, SaveOutcome::Saved { receipt } if column_receipt(receipt) && receipt.completion == SaveCompletion::Applied);
        if ordinary_success {
            self.acknowledge_number_column(identity);
        }
    }

    pub(in crate::workspace) fn number_column_save_command(
        &mut self,
        identity: NumberColumnSaveIdentity,
        command: NumberColumnSaveCommand,
    ) {
        let Some(host) = &self.editing.number_column else {
            return;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_number_column_save(&host.request, identity)
        {
            return;
        }
        match command {
            NumberColumnSaveCommand::Cancel => {
                let reopening = host.reopening;
                self.editing.number_column.as_mut().unwrap().cancelled = true;
                if reopening {
                    self.worker.cancel();
                } else {
                    self.editing.worker.cancel(identity.save_id);
                }
            }
            NumberColumnSaveCommand::Recover | NumberColumnSaveCommand::Release => {
                let recover = command == NumberColumnSaveCommand::Recover;
                if !self.state.can_resolve_number_column_save(identity, recover) {
                    return;
                }
                let result = if recover {
                    self.editing.worker.recover(identity.save_id)
                } else {
                    self.editing.worker.release(identity.save_id)
                };
                match result {
                    Ok(next_id) => {
                        let host = self.editing.number_column.as_mut().unwrap();
                        if let Some(next) = self.state.begin_number_column_resolution(
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
            NumberColumnSaveCommand::Acknowledge => self.acknowledge_number_column(identity),
            NumberColumnSaveCommand::RetryReopen => {
                if self.state.can_retry_number_column_reopen(identity) {
                    self.reopen_number_column(identity);
                }
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_number_column(
        &mut self,
        identity: NumberColumnSaveIdentity,
    ) {
        let Some(host) = &self.editing.number_column else {
            return;
        };
        if host.identity != Some(identity)
            || !self
                .state
                .matches_number_column_save(&host.request, identity)
            || !self.state.can_acknowledge_number_column_save(identity)
        {
            return;
        }
        let Some(save) = self.editing.save.as_ref().filter(|save| {
            save.id == identity.save_id && matches!(save.owner, SaveOwner::NumberColumn)
        }) else {
            return;
        };
        let Some(report) = save.report else {
            return;
        };
        let Ok((replacement, completed)) = reopen_facts(report.outcome) else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        if replacement && host.witness.is_none() {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        if !self.editing.worker.acknowledge(identity.save_id) {
            return;
        }
        self.editing.save = None;
        self.state.busy = false;
        if replacement {
            self.editing.number_column.as_mut().unwrap().cleanup =
                completed.and_then(cleanup_notice);
            self.reopen_number_column(identity);
        } else {
            let host = self.editing.number_column.as_ref().unwrap();
            self.state
                .release_number_column_save(&host.request, identity);
            self.editing.number_column = None;
            self.restore_number_column_close_after_refusal();
        }
    }

    fn reopen_number_column(&mut self, identity: NumberColumnSaveIdentity) {
        let Some(host) = &mut self.editing.number_column else {
            return;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || !host.terminal.is_some_and(|outcome| {
                reopen_facts(outcome).is_ok_and(|(replacement, _)| replacement)
            })
        {
            return;
        }
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        if !self.state.begin_number_column_reopen(identity) {
            return;
        }
        host.reopening = true;
        let request_id = host.request_id;
        self.submit(Command::OpenColumnAdded { witness });
        self.pending = PendingOperation::NumberColumnReopen { request_id };
    }

    pub(in crate::workspace) fn number_column_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .number_column
            .as_mut()
            .filter(|host| host.request_id == request_id && host.reopening)
        else {
            return;
        };
        host.reopening = false;
        if let Some(identity) = host.identity {
            self.state.fail_number_column_reopen(identity, message);
        }
    }

    /// Only the matching runtime full-content witness response reaches here.
    pub(in crate::workspace) fn finish_number_column_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.number_column else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        if host.request_id != request_id
            || !host.reopening
            || !host.terminal.is_some_and(|outcome| {
                reopen_facts(outcome).is_ok_and(|(replacement, _)| replacement)
            })
        {
            return;
        }
        let revision = database.document_revision;
        if !self.state.resolve_number_column_after_reopen(
            request_id,
            &host.request,
            identity.review_id,
            identity.save_id,
            database,
        ) {
            self.number_column_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let host = self.editing.number_column.as_mut().unwrap();
        self.state.clear_number_column_save(&host.request, identity);
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
        self.editing.number_column = None;
        self.resume_after_number_column = true;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }
}
