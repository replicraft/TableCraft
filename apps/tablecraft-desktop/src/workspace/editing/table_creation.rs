//! Host composition of shared draft requests and runtime-owned schema saves.
use super::{ActiveSave, SaveOwner, cleanup_notice, save_outcome};
use crate::workspace::{
    Desktop, PendingOperation,
    objects::{Sessions, WorkspaceRestore},
};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, CreatedTableReopen, ReviewedTableCreation, SaveKnownOutcome, SaveOperation,
    SaveOutcome, SaveReport, SaveSchemaChange, journal_path,
};
use tablecraft_ui::{Database, EditSaveOutcome, ObjectTab, SaveCleanupNotice, TableSaveRequest};

pub(in crate::workspace) struct TableCreationHost {
    request: u64,
    tab: ObjectTab,
    revision: u64,
    table: String,
    columns: usize,
    witness: Option<Arc<CreatedTableReopen>>,
    expected_schema_change: Option<SaveSchemaChange>,
    restore: Option<WorkspaceRestore>,
    no_write: bool,
    cleanup: Option<SaveCleanupNotice>,
}

impl Desktop {
    pub(in crate::workspace) fn save_table_creation(&mut self, request: TableSaveRequest) {
        if self.table_creation.is_some() {
            // A duplicate or later request must not reset the operation already
            // owned by this host, including its pending save/recovery status.
            return;
        }
        if self.editing.has_save() || self.creating.is_busy() || self.file_dialog.is_open() {
            // Shared UI has already issued this exact request. Refusing before
            // review has no storage effects, so return it to the naming phase
            // instead of leaving an issued request with no host owner.
            self.state.fail_table_draft_save(
                request.id,
                self.state.localizer.text("table-draft-save-failed"),
            );
            return;
        }
        let Some(restore) = self.capture_workspace_restore() else {
            self.state.fail_table_draft_save(
                request.id,
                self.state.localizer.text("objects-memory-limit"),
            );
            return;
        };
        if !self.state.begin_table_draft_save(&request) {
            return;
        }
        let id = request.id;
        self.table_creation = Some(TableCreationHost {
            request: id,
            revision: request.snapshot.revision(),
            table: request.snapshot.table_name().to_owned(),
            columns: request.snapshot.fields().len(),
            tab: request.tab.clone(),
            witness: None,
            expected_schema_change: None,
            restore: Some(restore),
            no_write: true,
            cleanup: None,
        });
        self.submit(Command::ReviewTableCreation {
            document_revision: request.tab.target.document_revision,
            snapshot: request.snapshot,
        });
        self.pending = PendingOperation::TableCreationReview { request_id: id };
    }

    pub(in crate::workspace) fn start_reviewed_table_creation(
        &mut self,
        generation: u64,
        capability: ReviewedTableCreation,
    ) {
        let Some(host) = self.table_creation.as_mut() else {
            return;
        };
        if generation != self.generation
            || !matches!(self.pending, PendingOperation::TableCreationReview { request_id } if request_id == host.request)
        {
            return;
        }
        let request = host.request;
        let witness = capability.reopen_witness();
        if witness.document_revision() != host.tab.target.document_revision
            || witness.draft_revision() != host.revision
            || witness.table_name() != host.table
            || witness.column_count() != host.columns
        {
            self.table_creation_review_failed(
                request,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let path = witness.source_path().to_path_buf();
        match self
            .editing
            .worker
            .start_table_creation(capability, journal_path(&path))
        {
            Ok(id) => {
                host.expected_schema_change = Some(witness.expected_schema_change());
                host.witness = Some(witness);
                host.no_write = false;
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::TableCreation,
                    id,
                    path,
                    table: host.table.clone(),
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                self.state.busy = true;
            }
            Err(error) => self.table_creation_review_failed(request, error.to_string()),
        }
    }

    pub(in crate::workspace) fn table_creation_review_failed(&mut self, id: u64, message: String) {
        if self
            .table_creation
            .as_ref()
            .is_some_and(|host| host.request == id)
        {
            self.table_creation = None;
            self.state.fail_table_draft_save(id, message);
        }
    }

    pub(super) fn table_creation_save_report(&mut self, report: SaveReport) {
        let Some(host) = &mut self.table_creation else {
            return;
        };
        let request = host.request;
        match report.outcome {
            SaveOutcome::Saved { receipt }
                if matches!(
                    receipt.known,
                    SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
                ) && host.expected_schema_change.is_some()
                    && receipt.schema_change == host.expected_schema_change =>
            {
                let Some(witness) = host.witness.as_ref().map(Arc::clone) else {
                    return;
                };
                if !self.editing.worker.acknowledge(report.id) {
                    return;
                }
                host.cleanup = cleanup_notice(receipt);
                self.editing.save = None;
                self.submit(Command::OpenTableCreated { witness });
                self.pending = PendingOperation::CreatedTableReopen {
                    request_id: request,
                };
            }
            SaveOutcome::Saved { receipt }
                if matches!(
                    receipt.known,
                    SaveKnownOutcome::RolledBack | SaveKnownOutcome::OriginalVerified
                ) =>
            {
                if self.editing.worker.acknowledge(report.id) {
                    self.editing.save = None;
                    self.table_creation = None;
                    self.state.cancel_table_draft_save(request);
                    if let Some(database) = &self.state.database {
                        self.state
                            .set_save_cleanup(database.document_revision, cleanup_notice(receipt));
                    }
                }
            }
            SaveOutcome::Failed(failure) => {
                if report.operation == SaveOperation::Apply {
                    host.no_write = report.stopped_before_target_writes();
                }
                let no_write = host.no_write;
                let message = report_message(self, report.outcome);
                if no_write && failure.state.is_none() && self.editing.worker.acknowledge(report.id)
                {
                    self.editing.save = None;
                    self.table_creation = None;
                    self.state.fail_table_draft_save(request, message);
                } else {
                    self.state.retain_table_draft_save_status(
                        request,
                        message,
                        failure.can_recover,
                        failure.can_release,
                        failure.state.is_none(),
                    );
                }
            }
            SaveOutcome::Released { .. } => {
                let no_write = host.no_write;
                let message = report_message(self, report.outcome);
                if no_write && self.editing.worker.acknowledge(report.id) {
                    self.editing.save = None;
                    self.table_creation = None;
                    self.state.fail_table_draft_save(request, message);
                } else {
                    self.state
                        .retain_table_draft_save_status(request, message, false, false, true);
                }
            }
            outcome => {
                let mut message = self.state.localizer.text("table-draft-save-retained");
                message.push(' ');
                message.push_str(&report_message(self, outcome));
                self.state
                    .retain_table_draft_save_status(request, message, false, false, true);
            }
        }
    }

    pub(in crate::workspace) fn finish_table_creation_reopen(
        &mut self,
        id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(mut host) = self.table_creation.take() else {
            return;
        };
        if host.request != id {
            self.table_creation = Some(host);
            return;
        }
        let Some(completion) = self.state.complete_table_draft_save(id, database) else {
            self.table_creation = Some(host);
            self.table_creation_reopen_failed(
                id,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        if let Some(database) = &self.state.database {
            let revision = database.document_revision;
            self.state
                .set_table_creation_available(revision, table_creation_available);
            self.state
                .set_table_design_available(revision, table_design_available);
            self.state.set_save_cleanup(revision, host.cleanup);
        }
        self.filter = None;
        self.objects = Sessions::default();
        if let Some(restore) = host.restore.take() {
            self.retain_workspace_restore(restore);
            self.restore_workspace_after_save(&host.table);
        }
        if completion.record_pending {
            self.resume_after_table_creation = completion.resume_transition;
            self.close_after_first_record = completion
                .close_after_save
                .then(|| completion.target.clone());
            self.open_object(completion.target);
        } else if completion.resume_transition {
            self.resume_after_table_creation = true;
        } else if !completion.close_after_save {
            self.open_object(completion.target);
        } else if let Some(open) = self.state.active_object_load() {
            self.load_object(open);
        }
    }

    pub(in crate::workspace) fn table_creation_reopen_failed(&mut self, id: u64, message: String) {
        if self
            .table_creation
            .as_ref()
            .is_some_and(|host| host.request == id)
        {
            let mut report = self.state.localizer.text("table-draft-save-reopen-failed");
            report.push(' ');
            report.push_str(&message);
            if self
                .table_creation
                .as_ref()
                .is_some_and(|host| host.cleanup.is_some())
            {
                report.push(' ');
                report.push_str(
                    &self
                        .state
                        .localizer
                        .text("editing-save-retained-private-journal"),
                );
            }
            self.state
                .retain_table_draft_save_status(id, report, false, false, true);
        }
    }

    pub(in crate::workspace) fn cancel_table_creation_save(&mut self, id: u64) {
        if !self
            .table_creation
            .as_ref()
            .is_some_and(|host| host.request == id)
        {
            return;
        }
        if let Some(save) = &self.editing.save {
            self.editing.worker.cancel(save.id);
        } else if matches!(self.pending, PendingOperation::TableCreationReview { .. }) {
            self.worker.cancel();
            self.table_creation = None;
            self.pending = PendingOperation::Read;
            self.state.cancel_table_draft_save(id);
        } else if matches!(self.pending, PendingOperation::CreatedTableReopen { .. }) {
            self.worker.cancel();
            self.pending = PendingOperation::Read;
            self.table_creation_reopen_failed(id, String::new());
        }
    }

    pub(in crate::workspace) fn cancel_active_table_creation_save(&mut self) {
        if let Some(host) = &self.table_creation {
            self.cancel_table_creation_save(host.request);
        }
    }

    pub(in crate::workspace) fn resolve_table_creation_save(&mut self, id: u64, recover: bool) {
        if !self
            .table_creation
            .as_ref()
            .is_some_and(|host| host.request == id)
        {
            return;
        }
        let Some(save) = &mut self.editing.save else {
            return;
        };
        let result = if recover {
            self.editing.worker.recover(save.id)
        } else {
            self.editing.worker.release(save.id)
        };
        match result {
            Ok(next) => {
                save.id = next;
                save.report = None;
                self.state.retain_table_draft_save_status(
                    id,
                    self.state.localizer.text("table-draft-save-working"),
                    false,
                    false,
                    false,
                );
            }
            Err(error) => {
                self.state.retain_table_draft_save_status(
                    id,
                    error.to_string(),
                    recover,
                    !recover,
                    false,
                );
            }
        }
    }

    pub(in crate::workspace) fn acknowledge_table_creation_save(&mut self, id: u64) {
        if !self
            .table_creation
            .as_ref()
            .is_some_and(|host| host.request == id)
        {
            return;
        }
        if let Some(save) = &self.editing.save
            && !self.editing.worker.acknowledge(save.id)
        {
            return;
        }
        if self.state.acknowledge_table_draft_save(id) {
            self.editing.save = None;
            self.table_creation = None;
        }
    }
}

fn report_message(desktop: &Desktop, outcome: SaveOutcome) -> String {
    match save_outcome(&desktop.state.localizer, outcome) {
        EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Failed(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Retained { message, .. } => message,
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod admission_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod test_support;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod creating_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod field_types_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod first_record_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod design_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod properties_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod numbers_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod addition_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod manual_tests;
