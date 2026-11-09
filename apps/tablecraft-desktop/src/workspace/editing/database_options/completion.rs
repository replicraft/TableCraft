//! Settings facts never publish a fresh document without their exact AFTER witness.
use super::*;
use crate::workspace::request_read;
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveReport};
use tablecraft_ui::Database;

impl Desktop {
    pub(in crate::workspace) fn database_options_save_report(&mut self, report: SaveReport) {
        let Some(host) = &mut self.editing.database_options else {
            return;
        };
        let Some(identity) = host
            .identity
            .filter(|identity| identity.save_id == report.id)
        else {
            return;
        };
        if !self.state.matches_database_options_save(identity) {
            return;
        }
        host.terminal = Some(report.outcome);
        self.state.set_database_options_save_outcome(
            identity,
            save_outcome(&self.state.localizer, report.outcome),
        );
        let ordinary = !host.cancelled
            && report.operation == SaveOperation::Apply
            && match report.outcome {
                SaveOutcome::Saved { receipt } => {
                    report
                        .outcome
                        .database_options_reopen_facts()
                        .is_ok_and(|facts| facts.replacement)
                        && receipt.completion == SaveCompletion::Applied
                }
                _ => false,
            };
        if ordinary {
            self.acknowledge_database_options(identity);
        }
    }

    pub(in crate::workspace) fn acknowledge_uncertain_database_options(
        &mut self,
        save_id: u64,
    ) -> bool {
        let Some(identity) = self
            .editing
            .database_options
            .as_ref()
            .and_then(|host| host.identity)
            .filter(|identity| identity.save_id == save_id)
        else {
            return false;
        };
        if !self.state.can_acknowledge_database_options_save(identity)
            || !self.editing.worker.acknowledge(save_id)
        {
            return false;
        }
        self.state
            .acknowledge_uncertain_database_options_save(identity);
        self.editing.database_options = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_database_options(&mut self, identity: Identity) {
        if self.acknowledge_uncertain_save(identity.save_id) {
            return;
        }

        let Some(host) = &self.editing.database_options else {
            return;
        };
        if host.identity != Some(identity)
            || !self.state.can_acknowledge_database_options_save(identity)
        {
            return;
        }
        let Some(save) = self.editing.save.as_ref().filter(|save| {
            save.id == identity.save_id && matches!(save.owner, SaveOwner::DatabaseOptions)
        }) else {
            return;
        };
        let Some(report) = save.report else {
            return;
        };
        let Ok(facts) = report.outcome.database_options_reopen_facts() else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        if facts.replacement && host.witness.is_none() {
            return;
        }
        if !self.editing.worker.acknowledge(identity.save_id) {
            return;
        }
        self.editing.save = None;
        self.state.busy = false;
        if facts.replacement {
            self.editing.database_options.as_mut().unwrap().cleanup =
                facts.completed.and_then(cleanup_notice);
            self.reopen_database_options(identity);
        } else {
            self.state.clear_database_options_save(identity);
            self.state.database_options_restore_settings();
            self.editing.database_options = None;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn reopen_database_options(&mut self, identity: Identity) {
        let Some(host) = &mut self.editing.database_options else {
            return;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || !host.terminal.is_some_and(|outcome| {
                outcome
                    .database_options_reopen_facts()
                    .is_ok_and(|facts| facts.replacement)
            })
        {
            return;
        }
        // NO_CLONE: Options host retains exact complete-image witness while the guarded options-
        // reopen read command owns a shared reference for retryable publication.
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        if !self.state.begin_database_options_reopen(identity) {
            return;
        }
        host.reopening = true;
        let request_id = host.request_id;
        let Some(generation) = request_read(
            Command::OpenDatabaseOptionsSaved { witness },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            let message = self
                .state
                .localizer
                .text("objects-read-identities-exhausted");
            self.database_options_reopen_failed(request_id, message);
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::DatabaseOptionsReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn database_options_reopen_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .database_options
            .as_mut()
            .filter(|host| host.request_id == request_id && host.reopening)
        else {
            return;
        };
        host.reopening = false;
        if let Some(identity) = host.identity {
            self.state.fail_database_options_reopen(identity, message);
        }
    }

    pub(in crate::workspace) fn finish_database_options_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.database_options else {
            return;
        };
        let Some(identity) = host.identity else {
            return;
        };
        if host.request_id != request_id
            || !host.reopening
            || !host.terminal.is_some_and(|outcome| {
                outcome
                    .database_options_reopen_facts()
                    .is_ok_and(|facts| facts.replacement)
            })
        {
            return;
        }
        let revision = database.document_revision;
        if !self
            .state
            .resolve_database_options_after_reopen(identity, database)
        {
            self.database_options_reopen_failed(
                request_id,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let host = self.editing.database_options.as_mut().unwrap();
        let cleanup = host.cleanup.take();
        // Confirmed persisted tabs are closed; old native filter handles/definitions retire.
        self.state.clear_database_options_save(identity);
        self.retain_filter_restore_definitions(Vec::new());
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, cleanup);
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        self.editing.database_options = None;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }
}
