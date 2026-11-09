//! Thin ownership of the runtime-issued rename and guarded catalog reopening.
use super::{ActiveSave, EditSaveOutcome, SaveOwner, cleanup_notice, save_outcome};
use crate::workspace::{Desktop, PendingOperation};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, ReviewedTableRename, SaveKnownOutcome, SaveOutcome, SaveReceipt, SaveReport,
    SaveSchemaChange, TableRenameReopen, TableRenameReview, journal_path,
};
use tablecraft_ui::{Database, ObjectKind, ObjectTarget, TableRenameRequest};

/// Host request and native expected-content witness for one rename lifecycle.
pub(in crate::workspace) struct TableRenameHost {
    request: TableRenameRequest,
    witness: Option<Arc<TableRenameReopen>>,
    cleanup: Option<tablecraft_ui::SaveCleanupNotice>,
    terminal: Option<SaveOutcome>,
}

impl Desktop {
    /// Submit the exact shared naming request without parsing or mutating native data.
    pub(in crate::workspace) fn rename_table(&mut self, request: TableRenameRequest) {
        if self.table_rename.is_some()
            || self.state.field_addition_source_stale()
            || !self.state.table_rename_request_current(&request)
        {
            return;
        }
        if self.editing.has_save() || self.creating.is_busy() || self.file_dialog.is_open() {
            self.state.fail_table_rename(
                request.id,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let command = Command::ReviewTableRename {
            document_revision: request.document_revision,
            table: request.table.clone(),
            new_name: request.new_name.clone(),
        };
        let request_id = request.id;
        self.table_rename = Some(TableRenameHost {
            request,
            witness: None,
            cleanup: None,
            terminal: None,
        });
        self.submit(command);
        self.pending = PendingOperation::TableRenameReview { request_id };
    }

    /// Accept only the current runtime review and transfer its capability to the save worker.
    pub(in crate::workspace) fn start_reviewed_table_rename(
        &mut self,
        generation: u64,
        review: TableRenameReview,
        capability: ReviewedTableRename,
    ) {
        let Some(host) = &mut self.table_rename else {
            return;
        };
        if generation != self.generation
            || !matches!(self.pending, PendingOperation::TableRenameReview { request_id } if request_id == host.request.id)
            || !self.state.table_rename_request_current(&host.request)
        {
            return;
        }
        let witness = capability.reopen_witness();
        if review.document_revision != host.request.document_revision
            || review.table != host.request.table
            || review.new_name != host.request.new_name
            || review.changed != (host.request.table != host.request.new_name)
            || witness.document_revision() != host.request.document_revision
            || witness.original_name() != host.request.table
            || witness.table_name() != host.request.new_name
            || capability.source_path() != witness.source_path()
        {
            let id = host.request.id;
            self.table_rename_review_failed(id, self.state.localizer.text("objects-open-refused"));
            return;
        }
        if !review.changed {
            self.state.finish_unchanged_table_rename(&host.request);
            self.table_rename = None;
            return;
        }
        let path = capability.source_path().to_path_buf();
        match self
            .editing
            .worker
            .start_table_rename(capability, journal_path(&path))
        {
            Ok(id) => {
                self.state.begin_table_rename_save(&host.request, id);
                host.witness = Some(witness);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::TableRename,
                    id,
                    path,
                    table: host.request.new_name.clone(),
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                self.state.busy = true;
            }
            Err(error) => {
                let request = host.request.id;
                self.table_rename_review_failed(request, error.to_string());
            }
        }
    }

    /// Cancel only an unaccepted matching review; an active save keeps its owner.
    pub(in crate::workspace) fn cancel_table_rename_review(&mut self, request_id: u64) {
        if self.editing.has_save()
            || !self
                .table_rename
                .as_ref()
                .is_some_and(|host| host.request.id == request_id)
        {
            return;
        }
        if self.state.cancel_table_rename_review(request_id) {
            self.table_rename = None;
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    /// Return a matching review failure to its original naming editor.
    pub(in crate::workspace) fn table_rename_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        if self
            .table_rename
            .as_ref()
            .is_some_and(|host| host.request.id == request_id)
        {
            self.table_rename = None;
            self.state.fail_table_rename(request_id, message);
        }
    }

    /// Publish the real save result through the shared retained-outcome presentation.
    pub(in crate::workspace) fn table_rename_save_report(&mut self, report: SaveReport) {
        self.state.set_table_rename_save_outcome(
            report.id,
            save_outcome(&self.state.localizer, report.outcome),
        );
    }

    /// Acknowledge a terminal save and explicitly reopen only its matching committed image.
    pub(in crate::workspace) fn acknowledge_table_rename(&mut self, id: u64) {
        let Some(save) = &self.editing.save else {
            return;
        };
        if save.id != id || !self.state.can_acknowledge_table_rename_save(id) {
            return;
        }
        let Some(report) = save.report else {
            return;
        };
        let (replacement, completed) = rename_replacement(report.outcome);
        if matches!(report.outcome, SaveOutcome::Saved { receipt } if known_replacement(receipt.known))
            && !replacement
        {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        if let (Some(host), Some(receipt)) = (&mut self.table_rename, completed) {
            host.cleanup = cleanup_notice(receipt);
        }
        let witness = self
            .table_rename
            .as_ref()
            .and_then(|host| host.witness.as_ref())
            .map(Arc::clone);
        if replacement && witness.is_none() {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        if !self.editing.worker.acknowledge(id) {
            return;
        }
        self.editing.save = None;
        self.state.acknowledge_table_rename_save(id);
        self.state.busy = false;
        if replacement && let Some(witness) = witness {
            self.table_rename.as_mut().unwrap().terminal = Some(report.outcome);
            let request_id = self.table_rename.as_ref().unwrap().request.id;
            self.submit(Command::OpenTableRenamed { witness });
            self.pending = PendingOperation::RenamedTableReopen { request_id };
        } else {
            self.table_rename = None;
        }
    }

    /// Preserve known terminal facts if the expected-content read cannot reopen.
    pub(in crate::workspace) fn fail_table_rename_reopen(
        &mut self,
        error: &tablecraft_runtime::Error,
    ) {
        let outcome = self.table_rename.take().and_then(|host| host.terminal);
        let message = outcome.map(
            |outcome| match save_outcome(&self.state.localizer, outcome) {
                EditSaveOutcome::Saved(message)
                | EditSaveOutcome::Cancelled(message)
                | EditSaveOutcome::Failed(message)
                | EditSaveOutcome::Retained { message, .. } => message,
            },
        );
        self.state.database = None;
        self.state.data = None;
        self.state.query_data = None;
        self.state.clear_schema();
        self.state.error = Some(match message {
            Some(message) => format!("{message}\n{error}"),
            None => error.to_string(),
        });
        self.state.show_open();
    }

    /// Install a guarded fresh catalog and select the renamed entry while leaving it closed.
    pub(in crate::workspace) fn finish_table_rename_reopen(
        &mut self,
        request_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        let Some(host) = self.table_rename.take() else {
            return;
        };
        if host.request.id != request_id {
            return;
        }
        let revision = database.document_revision;
        let name = host.request.new_name;
        self.objects = Default::default();
        self.filter = None;
        self.state.database = Some(database);
        self.state.data = None;
        self.state.query_data = None;
        self.state.schema = None;
        self.state
            .set_table_creation_available(revision, table_creation_available);
        self.state
            .set_table_design_available(revision, table_design_available);
        self.state.dismiss_backstage();
        self.state.set_save_cleanup(revision, host.cleanup);
        self.state.select_renamed_table(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table { name },
        });
    }
}

fn known_replacement(known: SaveKnownOutcome) -> bool {
    matches!(
        known,
        SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
    )
}

fn rename_receipt(receipt: SaveReceipt) -> bool {
    known_replacement(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::TableRenamed)
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

// Released current facts take precedence over an earlier finalized receipt.
// A current replacement may lack a receipt after namespace finalization failed;
// the same accepted host's exact AFTER witness authorizes only a guarded read.
// No missing schema, retirement or cleanup receipt is manufactured here.
fn rename_replacement(outcome: SaveOutcome) -> (bool, Option<SaveReceipt>) {
    match outcome {
        SaveOutcome::Saved { receipt } => {
            let qualifies = rename_receipt(receipt);
            (qualifies, qualifies.then_some(receipt))
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => {
            let completed = last_completed.filter(|receipt| rename_receipt(*receipt));
            let replacement = match current_outcome {
                Some(known) => known_replacement(known),
                None => completed.is_some(),
            };
            (replacement, completed)
        }
        SaveOutcome::Failed(_) => (false, None),
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
