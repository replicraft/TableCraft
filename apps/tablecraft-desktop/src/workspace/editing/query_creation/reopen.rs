//! Only the acknowledged creation's complete-image witness may publish its object.
use super::super::save_outcome;
use crate::workspace::{Desktop, PendingOperation, request_read};
use std::sync::Arc;
use tablecraft_runtime::Command;
use tablecraft_ui::{Database, EditSaveOutcome};

impl Desktop {
    pub(in crate::workspace) fn reopen_query_creation(&mut self) {
        let Some(host) = &mut self.editing.query_creation else {
            return;
        };
        if self.editing.save.is_some() || !host.witnessed_reopen_authorized() || host.reopening {
            return;
        }
        // NO_CLONE: The host retains the immutable full-image witness for read-only
        // retry; the read worker separately owns its command reference.
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        let Some(status) = host.status else {
            return;
        };
        if !self.state.begin_query_creation_reopen(status) {
            return;
        }
        host.reopening = true;
        let Some(generation) = request_read(
            Command::OpenQueryCreated { witness },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            self.fail_query_creation_reopen(&tablecraft_runtime::Error::ReadRequestExhausted(
                tablecraft_runtime::ReadRequestExhausted,
            ));
            self.state.busy = false;
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::QueryCreationReopen {
            request_id: host.request_id,
        };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn fail_query_creation_reopen(
        &mut self,
        error: &tablecraft_runtime::Error,
    ) {
        let Some(host) = &mut self.editing.query_creation else {
            return;
        };
        host.reopening = false;
        if let Some(status) = host.status {
            self.state
                .set_query_creation_reopen_failed(status, error.to_string());
        }
        self.state.busy = true;
    }

    pub(in crate::workspace) fn finish_query_creation_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.query_creation else {
            return;
        };
        let Some(status) = host.status else {
            return;
        };
        if host.request_id != request_id
            || !host.reopening
            || !host.witnessed_reopen_authorized()
            || database.document_revision <= host.request.document_revision
        {
            return;
        }
        // OpenQueryCreated has independently verified the signed native ID,
        // type/flags/name and complete AFTER image before this catalog is returned.
        let Some(query) = database
            .queries
            .iter()
            .find(|query| !query.system && query.name == host.request.destination)
        else {
            self.fail_query_creation_reopen(&tablecraft_runtime::Error::QueryCreatedSourceChanged);
            return;
        };
        // NO_CLONE: The new saved target and complete fresh catalog are independent
        // UI owners; publishing the catalog must retain its own object name.
        let created = tablecraft_ui::SavedQueryTarget {
            document_revision: database.document_revision,
            object_id: query.object_id,
            name: query.name.clone(),
        };
        let revision = database.document_revision;
        if !self
            .state
            .resolve_query_creation_reopen(status, database, created)
        {
            let Some(host) = self.editing.query_creation.as_mut() else {
                self.state.error = Some(self.state.localizer.text("objects-open-refused"));
                return;
            };
            host.reopening = false;
            self.state.set_query_creation_reopen_failed(
                status,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        self.state.clear_query_creation_save(status);
        let Some(host) = self.editing.query_creation.take() else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        self.retain_filter_restore_definitions(host.filters);
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, host.cleanup);
        self.state.dismiss_backstage();
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    /// Returning to authoring preserves acknowledged historical facts without
    /// claiming that a failed fresh read restored the old stored definition.
    pub(in crate::workspace) fn keep_query_creation_draft_after_reopen_failure(&mut self) -> bool {
        let Some(host) = &self.editing.query_creation else {
            return false;
        };
        let (Some(review_id), Some(status)) = (host.review_id, host.status) else {
            return false;
        };
        if self.editing.save.is_some()
            || host.reopening
            || !host.witnessed_reopen_authorized()
            || !self.state.can_return_to_query_creation_draft(status)
        {
            return false;
        }
        if !self
            .state
            .matches_query_creation_review(host.request_id, &host.request, review_id)
        {
            return false;
        }
        // NO_CLONE: Releasing the retained status below destroys its error owner;
        // the separately published warning must retain those exact bytes.
        let read_error = self
            .state
            .query_creation_reopen_error(status)
            .filter(|message| !message.is_empty())
            .map_or_else(
                || self.state.localizer.text("objects-memory-limit"),
                str::to_owned,
            );
        let warning =
            host.terminal.map(
                |outcome| match save_outcome(&self.state.localizer, outcome) {
                    EditSaveOutcome::Saved(message)
                    | EditSaveOutcome::Failed(message)
                    | EditSaveOutcome::Cancelled(message)
                    | EditSaveOutcome::Retained { message, .. } => message,
                },
            );
        self.state.error = Some(match warning {
            Some(message) => format!("{message}\n{read_error}"),
            None => read_error,
        });
        if let Some(revision) = self
            .state
            .database
            .as_ref()
            .map(|database| database.document_revision)
        {
            self.state.set_save_cleanup(revision, host.cleanup);
        }
        self.state.clear_query_creation_save(status);
        self.editing.query_creation = None;
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        true
    }
}
