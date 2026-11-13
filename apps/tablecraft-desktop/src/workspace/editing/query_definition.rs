//! One exact saved-SQL request across runtime review, save and witnessed reopening.
use super::{ActiveSave, SaveOwner, cleanup_notice, save_outcome};
use crate::workspace::request_read;
use crate::workspace::{Desktop, PendingOperation, filtering::RestoreFilter};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, QueryExpressionReopen, QuerySqlReview, ReviewedQuerySql, SaveCompletion,
    SaveOperation, SaveOutcome, SaveReport, journal_path,
};
use tablecraft_ui::{
    Database, EditSaveOutcome, SaveCleanupNotice, SavedQuerySqlRequest, SavedQuerySqlSaveCommand,
    SavedQuerySqlSaveIdentity, SavedQuerySqlSaveOperation,
};

/// This owner is not cleared by unrelated record reviews or presentation changes.
/// UI text remains owned by its original session until an exact resolved reopen.
pub(in crate::workspace) struct QueryDefinitionHost {
    request_id: u64,
    request: SavedQuerySqlRequest,
    review_id: Option<u64>,
    witness: Option<Arc<QueryExpressionReopen>>,
    status: Option<SavedQuerySqlSaveIdentity>,
    terminal: Option<SaveOutcome>,
    cleanup: Option<SaveCleanupNotice>,
    reopening: bool,
    close_after_save: bool,
    filters: Vec<RestoreFilter>,
    reopen_started: bool,
    unchanged_review: bool,
    cancellation_requested: bool,
}

impl QueryDefinitionHost {
    fn witnessed_reopen_authorized(&self) -> bool {
        self.unchanged_review
            || self.terminal.is_some_and(|outcome| {
                outcome
                    .query_expression_reopen_facts()
                    .is_ok_and(|facts| facts.replacement)
            })
    }
}

impl Desktop {
    /// Review the supplied complete text without host SQL parsing or rewriting.
    /// Rebind preflight belongs to the shared object owner, not tab-order guesses.
    pub(in crate::workspace) fn save_query_definition(&mut self, request: SavedQuerySqlRequest) {
        if self.editing.has_save()
            || self.state.native_mutation_blocked()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.saved_query_sql_request_current(&request)
            || !self.state.can_rebind_saved_query_sql_layout(&request)
        {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        let Some(filters) = self.capture_filter_restore_definitions() else {
            return;
        };
        let Some(original_sql) = request.original_sql else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        let target = tablecraft_runtime::SavedQueryTarget {
            document_revision: request.target.document_revision,
            object_id: request.target.object_id,
            // NO_CLONE: Runtime saved-query review consumes SQL/target while QueryDefinitionHost
            // retains the exact object target for reply checks, close ownership and full-
            // image publication.
            name: request.target.name.clone(),
        };
        // Move the already issued outgoing strings; the shared owner separately
        // accounts for its retained pending snapshot and this returned binding.
        let Some(request_id) = request_read(
            Command::ReviewQuerySql {
                target,
                original_sql,
                proposed_sql: request.proposed_sql,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        let Some(request) = self.state.begin_saved_query_sql_review(request_id) else {
            self.worker.cancel();
            return;
        };
        self.editing.query_definition = Some(QueryDefinitionHost {
            request_id,
            request,
            review_id: None,
            witness: None,
            status: None,
            terminal: None,
            cleanup: None,
            reopening: false,
            close_after_save: false,
            filters,
            reopen_started: false,
            unchanged_review: false,
            cancellation_requested: false,
        });
        self.generation = request_id;
        self.pending = PendingOperation::QueryDefinitionReview { request_id };
        self.state.busy = true;
        self.state.error = None;
    }

    /// The shared close choice has already bound the exact object and text.
    pub(in crate::workspace) fn save_query_definition_for_close(
        &mut self,
        request: SavedQuerySqlRequest,
    ) {
        if self.editing.has_save() || !self.state.saved_query_sql_request_current(&request) {
            return;
        }
        self.save_query_definition(request);
        if let Some(host) = &mut self.editing.query_definition {
            host.close_after_save = true;
        } else {
            self.cancel_table_draft_action();
        }
    }

    /// An exact runtime response supplies the capability; request data never does.
    pub(in crate::workspace) fn start_reviewed_query_definition(
        &mut self,
        generation: u64,
        review: QuerySqlReview,
        capability: ReviewedQuerySql,
    ) {
        let Some(host) = &self.editing.query_definition else {
            return;
        };
        if generation != self.generation
            || generation != host.request_id
            || !matches!(self.pending, PendingOperation::QueryDefinitionReview { request_id } if request_id == generation)
        {
            return;
        }
        let request = &host.request;
        let witness = capability.reopen_witness();
        let exact = review.target.document_revision == request.target.document_revision
            && review.target.object_id == request.target.object_id
            && review.target.name == request.target.name
            && Some(review.original_sql.as_str()) == request.original_sql.as_deref()
            && review.proposed_sql == request.proposed_sql
            && witness.document_revision() == request.target.document_revision
            && witness.query_name() == request.target.name
            && witness.native_id() == review.native_id
            && witness.source_path() == capability.source_path()
            && self.state.can_rebind_saved_query_sql_layout(request);
        if !exact
            || !self
                .state
                .set_saved_query_sql_review(generation, request, review.id)
        {
            self.query_definition_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let host = self.editing.query_definition.as_mut().unwrap();
        host.review_id = Some(review.id);
        host.witness = Some(witness);
        if !review.changed {
            let Some(status) = self.state.begin_saved_query_sql_unchanged_reopen(
                generation,
                &host.request,
                review.id,
            ) else {
                self.query_definition_review_failed(
                    generation,
                    self.state.localizer.text("objects-memory-limit"),
                );
                return;
            };
            host.status = Some(status);
            host.unchanged_review = true;
            drop(capability);
            self.reopen_query_definition();
            return;
        }
        let Some(prepared) =
            self.state
                .prepare_saved_query_sql_save(generation, &host.request, review.id)
        else {
            self.query_definition_review_failed(
                generation,
                self.state.localizer.text("objects-memory-limit"),
            );
            return;
        };
        host.status = Some(prepared);
        let path = capability.source_path().to_path_buf();
        match self
            .editing
            .worker
            .start_query_sql(capability, journal_path(&path))
        {
            Ok(id) => {
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::QueryDefinition,
                    id,
                    path,
                    table: String::new(),
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                if let Some(status) = self.state.bind_saved_query_sql_save(prepared, id) {
                    self.editing.query_definition.as_mut().unwrap().status = Some(status);
                } else {
                    // The accepted native owner is retained even if presentation
                    // binding detects an invariant failure. Never lose its report.
                    self.editing.worker.cancel(id);
                    self.state.error = Some(self.state.localizer.text("objects-open-refused"));
                }
                self.state.busy = true;
            }
            Err(error) => {
                self.state.abort_saved_query_sql_save(prepared);
                self.editing.query_definition.as_mut().unwrap().status = None;
                let message = error.to_string();
                // The rejected capability is returned by the worker and dropped
                // here deliberately. It is never silently retried.
                drop(error.into_reviewed());
                self.query_definition_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn query_definition_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .query_definition
            .as_ref()
            .filter(|host| host.request_id == request_id)
        else {
            return;
        };
        let mut accepted = if let Some(review_id) = host.review_id {
            self.state.release_saved_query_sql_review(
                request_id,
                &host.request,
                review_id,
                Some(message),
            )
        } else {
            self.state
                .fail_saved_query_sql(request_id, &host.request, message)
        };
        if !accepted {
            accepted = if let Some(review_id) = host.review_id {
                self.state.release_saved_query_sql_review(
                    request_id,
                    &host.request,
                    review_id,
                    None,
                )
            } else {
                self.state
                    .fail_saved_query_sql(request_id, &host.request, String::new())
            };
            if accepted {
                self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            }
        }
        if accepted {
            if let Some(status) = host.status {
                self.state.clear_saved_query_sql_save(status);
            }
            let cancel_transition = host.close_after_save;
            self.editing.query_definition = None;
            if cancel_transition {
                self.cancel_table_draft_action();
            }
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn query_definition_save_report(&mut self, report: SaveReport) {
        let Some(status) = self
            .editing
            .query_definition
            .as_ref()
            .and_then(|host| host.status)
        else {
            return;
        };
        if !matches!(status.operation, SavedQuerySqlSaveOperation::NativeSave { save_id } if save_id == report.id)
        {
            return;
        }
        if !self.state.set_saved_query_sql_save_outcome(
            status,
            save_outcome(&self.state.localizer, report.outcome),
        ) {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        // Ordinary Save completes without a success confirmation. A receipt
        // still cannot clear the draft: internal acknowledgement must start the
        // same complete-image witnessed read and atomic workspace publication.
        let ordinary = self
            .editing
            .query_definition
            .as_ref()
            .is_some_and(|host| !host.cancellation_requested)
            && self.editing.save.as_ref().is_some_and(|save| {
                save.id == report.id
                    && matches!(save.owner, SaveOwner::QueryDefinition)
                    && save.report == Some(report)
            })
            && ordinary_applied_query_save(report);
        if ordinary {
            self.acknowledge_query_definition(report.id);
        }
    }

    pub(in crate::workspace) fn note_query_definition_save_cancellation(&mut self, id: u64) {
        if self
            .editing
            .save
            .as_ref()
            .is_some_and(|save| save.id == id && matches!(save.owner, SaveOwner::QueryDefinition))
            && let Some(host) = &mut self.editing.query_definition
        {
            host.cancellation_requested = true;
        }
    }

    pub(in crate::workspace) fn query_definition_save_action(
        &mut self,
        identity: SavedQuerySqlSaveIdentity,
        command: SavedQuerySqlSaveCommand,
    ) {
        if !self
            .editing
            .query_definition
            .as_ref()
            .is_some_and(|host| host.status == Some(identity))
            || !self.state.matches_saved_query_sql_save(identity)
        {
            return;
        }
        match command {
            SavedQuerySqlSaveCommand::Cancel => {
                if let Some(save) = &self.editing.save {
                    self.cancel_save(save.id);
                } else {
                    self.cancel_query_definition_operation();
                }
            }
            SavedQuerySqlSaveCommand::Recover | SavedQuerySqlSaveCommand::Release => {
                if let SavedQuerySqlSaveOperation::NativeSave { save_id } = identity.operation {
                    self.resolve_save(save_id, command == SavedQuerySqlSaveCommand::Recover);
                }
            }
            SavedQuerySqlSaveCommand::Acknowledge => {
                if self.state.can_acknowledge_saved_query_sql_save(identity)
                    && let SavedQuerySqlSaveOperation::NativeSave { save_id } = identity.operation
                {
                    self.acknowledge_query_definition(save_id);
                }
            }
            SavedQuerySqlSaveCommand::RetryReopen => self.reopen_query_definition(),
            SavedQuerySqlSaveCommand::ReturnToDraft => {
                self.keep_query_definition_dirty_after_reopen_failure();
            }
        }
    }

    pub(in crate::workspace) fn can_resolve_query_definition_save(
        &self,
        id: u64,
        recover: bool,
    ) -> bool {
        let Some(status) = self
            .editing
            .query_definition
            .as_ref()
            .and_then(|host| host.status)
        else {
            return false;
        };
        if !self.state.can_resolve_saved_query_sql_save(status, recover) {
            return false;
        }
        let Some(save) = self.editing.save.as_ref() else {
            return false;
        };
        if save.id != id || !matches!(save.owner, SaveOwner::QueryDefinition) {
            return false;
        }
        let Some(report) = save.report else {
            return false;
        };
        let SaveOutcome::Failed(failure) = report.outcome else {
            return false;
        };
        if recover {
            failure.can_recover
        } else {
            failure.can_release
        }
    }

    pub(in crate::workspace) fn query_definition_resolution_started(&mut self, next: u64) {
        let Some(host) = &mut self.editing.query_definition else {
            return;
        };
        let Some(status) = host.status else {
            return;
        };
        match self
            .state
            .begin_saved_query_sql_save_resolution(status, next)
        {
            Some(next) => host.status = Some(next),
            None => self.state.error = Some(self.state.localizer.text("objects-open-refused")),
        }
        self.state.busy = true;
    }

    /// A receipt alone cannot resolve dirty text or publish a fresh revision.
    pub(in crate::workspace) fn acknowledge_uncertain_query_definition(&mut self, id: u64) -> bool {
        let Some(host) = &self.editing.query_definition else {
            return false;
        };
        let (Some(review_id), Some(identity)) = (host.review_id, host.status) else {
            return false;
        };
        if !self
            .state
            .matches_saved_query_sql_review(host.request_id, &host.request, review_id)
            || !self.state.can_acknowledge_saved_query_sql_save(identity)
            || !self.editing.worker.acknowledge(id)
        {
            return false;
        }
        // This API releases only pending presentation; its contract makes no
        // claim about native resources or persisted original/replacement bytes.
        self.state
            .release_saved_query_sql_review(host.request_id, &host.request, review_id, None);
        self.state.clear_saved_query_sql_save(identity);
        self.editing.query_definition = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_query_definition(&mut self, id: u64) {
        if self.acknowledge_uncertain_save(id) {
            return;
        }

        if self.editing.save.is_none() {
            if self
                .editing
                .query_definition
                .as_ref()
                .is_some_and(|host| matches!(host.status.map(|status| status.operation), Some(SavedQuerySqlSaveOperation::NativeSave { save_id }) if save_id == id))
            {
                self.keep_query_definition_dirty_after_reopen_failure();
            }
            return;
        }
        let Some(save) = self
            .editing
            .save
            .as_ref()
            .filter(|save| save.id == id && matches!(save.owner, SaveOwner::QueryDefinition))
        else {
            return;
        };
        let Some(report) = save.report else {
            return;
        };
        let Some(host) = &self.editing.query_definition else {
            return;
        };
        let Some(review_id) = host.review_id else {
            return;
        };
        if !self
            .state
            .matches_saved_query_sql_review(host.request_id, &host.request, review_id)
        {
            return;
        }
        let facts = match report.outcome.query_expression_reopen_facts() {
            Ok(facts) => facts,
            Err(error) => {
                self.state.error = Some(error.to_string());
                return;
            }
        };
        let is_replacement = facts.replacement;
        let completed = facts.completed;
        if is_replacement && host.witness.is_none() {
            return;
        }
        if !self.editing.worker.acknowledge(id) {
            return;
        }
        self.editing.save = None;
        let host = self.editing.query_definition.as_mut().unwrap();
        host.terminal = Some(report.outcome);
        host.cleanup = completed.and_then(cleanup_notice);
        if is_replacement {
            self.reopen_query_definition();
        } else {
            let released = self.state.release_saved_query_sql_review(
                host.request_id,
                &host.request,
                review_id,
                None,
            );
            if released {
                if let Some(status) = host.status {
                    self.state.clear_saved_query_sql_save(status);
                }
                let cancel_transition = host.close_after_save;
                self.editing.query_definition = None;
                if cancel_transition {
                    self.cancel_table_draft_action();
                }
                self.state.busy = false;
                self.pending = PendingOperation::Read;
            }
        }
    }

    /// Explicit read-only retry; it never constructs or replays another edit.
    pub(in crate::workspace) fn reopen_query_definition(&mut self) {
        let Some(host) = &mut self.editing.query_definition else {
            return;
        };
        if self.editing.save.is_some() || !host.witnessed_reopen_authorized() || host.reopening {
            return;
        }
        // NO_CLONE: Saved-query host retains immutable expected-content witness for retry while a
        // separate read command owns its guarded reopening reference.
        let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
            return;
        };
        let Some(status) = host.status else {
            return;
        };
        if (!host.unchanged_review || host.reopen_started)
            && !self.state.begin_saved_query_sql_reopen(status)
        {
            return;
        }
        host.reopen_started = true;
        host.reopening = true;
        let Some(generation) = request_read(
            Command::OpenQueryExpressionSaved { witness },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            self.fail_query_definition_reopen(&tablecraft_runtime::Error::ReadRequestExhausted(
                tablecraft_runtime::ReadRequestExhausted,
            ));
            self.state.busy = false;
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::QueryDefinitionReopen {
            request_id: host.request_id,
        };
        self.state.busy = true;
    }

    /// Keep every old session, baseline, proposed string and revision on failure.
    pub(in crate::workspace) fn fail_query_definition_reopen(
        &mut self,
        error: &tablecraft_runtime::Error,
    ) {
        let Some(host) = &mut self.editing.query_definition else {
            return;
        };
        host.reopening = false;
        let message = error.to_string();
        if let Some(status) = host.status {
            self.state
                .set_saved_query_sql_reopen_failed(status, message);
        }
        self.state.busy = true;
    }

    /// Called only by the exact successful full-witness runtime reopen response.
    pub(in crate::workspace) fn finish_query_definition_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = &self.editing.query_definition else {
            return;
        };
        let Some(review_id) = host.review_id else {
            return;
        };
        if host.request_id != request_id || !host.reopening || !host.witnessed_reopen_authorized() {
            return;
        }
        let revision = database.document_revision;
        if !self.state.resolve_saved_query_sql_review_after_reopen(
            host.request_id,
            &host.request,
            review_id,
            database,
        ) {
            let host = self.editing.query_definition.as_mut().unwrap();
            host.reopening = false;
            if let Some(status) = host.status {
                self.state.set_saved_query_sql_reopen_failed(
                    status,
                    self.state.localizer.text("objects-restore-refused"),
                );
            }
            return;
        }
        let design_close = self.state.finish_query_design_close_after_saved_sql(
            host.request_id,
            &host.request,
            review_id,
        );
        if let Some(status) = host.status {
            self.state.clear_saved_query_sql_save(status);
        }
        let host = self.editing.query_definition.take().unwrap();
        let cleanup = host.cleanup;
        let close_after_save = host.close_after_save;
        let close_object_id = host.request.target.object_id;
        let close_name = host.request.target.name;
        let filters = host.filters;
        self.retain_filter_restore_definitions(filters);
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.set_save_cleanup(revision, cleanup);
        self.state.dismiss_backstage();
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        self.editing.clear_review();
        self.clear_query_parameters();
        if let Some(tab) = design_close {
            self.close_object(tab);
            self.resume_after_query_definition = true;
        } else if close_after_save {
            if let Some(tab) = self.state.object_tabs().into_iter().find(|tab| {
                tab.target.document_revision == revision
                    && matches!(&tab.target.kind, tablecraft_ui::ObjectKind::SavedQuery { object_id, name }
                        if *object_id == close_object_id && *name == close_name)
            }) {
                self.close_object(tab);
            }
            self.resume_after_query_definition = true;
        } else if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    /// Explicitly return failed witnessed reopening to the unchanged dirty draft.
    /// This never asserts that the stored query is original or marks text saved.
    pub(in crate::workspace) fn keep_query_definition_dirty_after_reopen_failure(
        &mut self,
    ) -> bool {
        let Some(host) = &self.editing.query_definition else {
            return false;
        };
        let Some(review_id) = host.review_id else {
            return false;
        };
        if self.editing.save.is_some()
            || host.reopening
            || !host.witnessed_reopen_authorized()
            || !host
                .status
                .is_some_and(|identity| self.state.can_return_to_saved_query_sql_draft(identity))
        {
            return false;
        }
        if !self.state.release_saved_query_sql_review(
            host.request_id,
            &host.request,
            review_id,
            None,
        ) {
            return false;
        }
        let read_error = host
            .status
            .and_then(|identity| self.state.saved_query_sql_reopen_error(identity))
            .filter(|message| !message.is_empty())
            .map_or_else(
                || self.state.localizer.text("objects-memory-limit"),
                str::to_owned,
            );
        // These are historical acknowledged facts. A failed fresh read cannot
        // turn them into rollback/original claims or mark the visible text saved.
        let warning =
            host.terminal.map(
                |outcome| match save_outcome(&self.state.localizer, outcome) {
                    EditSaveOutcome::Saved(message)
                    | EditSaveOutcome::Failed(message)
                    | EditSaveOutcome::Cancelled(message)
                    | EditSaveOutcome::Retained { message, .. } => message,
                },
            );
        let warning = match warning {
            Some(message) => format!("{message}\n{read_error}"),
            None => read_error,
        };
        let revision = self
            .state
            .database
            .as_ref()
            .map(|database| database.document_revision);
        if let (Some(revision), Some(cleanup)) = (revision, host.cleanup) {
            self.state.set_save_cleanup(revision, Some(cleanup));
        }
        self.state.error = Some(warning);
        if let Some(status) = host.status {
            self.state.clear_saved_query_sql_save(status);
        }
        let cancel_transition = host.close_after_save;
        self.editing.query_definition = None;
        if cancel_transition {
            self.cancel_table_draft_action();
        }
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        true
    }

    /// Cancel an unlocked review or active read; a retained save stays worker-owned.
    pub(in crate::workspace) fn cancel_query_definition_operation(&mut self) -> bool {
        let Some(host) = &self.editing.query_definition else {
            return false;
        };
        if self.editing.save.is_some() {
            return false;
        }
        if host.review_id.is_none() {
            let request_id = host.request_id;
            self.worker.cancel();
            self.query_definition_review_failed(
                request_id,
                self.state.localizer.text("shell-operation-cancelled"),
            );
        } else if host.reopening {
            self.worker.cancel();
            self.fail_query_definition_reopen(&tablecraft_runtime::Error::Cancelled);
        }
        true
    }
}

// Recovery, release, cancellation and unqualified completion remain explicit.
// Reuse the runtime-owned query-only receipt check rather than interpreting text.
fn ordinary_applied_query_save(report: SaveReport) -> bool {
    report.operation == SaveOperation::Apply
        && matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.completion == SaveCompletion::Applied)
        && report
            .outcome
            .query_expression_reopen_facts()
            .is_ok_and(|facts| facts.replacement)
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
