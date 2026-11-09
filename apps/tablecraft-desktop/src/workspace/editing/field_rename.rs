//! Typed field rename composition; admission and phase rules remain shared/runtime-owned.
mod completion;
mod inspection;
#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod widgets;

use super::{
    ActiveSave, SaveOwner,
    field_save::{FieldReadWitness, FieldSaveHost},
};
use crate::workspace::request_read;
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{
    Command, FieldRenameReview, FieldRenamedReopen, ReviewedFieldRename, journal_path,
};
use tablecraft_ui::{FieldRenameRequest, FieldRenameSaveIdentity, ObjectKind};

pub(in crate::workspace) type FieldRenameHost =
    FieldSaveHost<FieldRenameRequest, FieldRenamedReopen, FieldRenameSaveIdentity>;

impl Desktop {
    pub(in crate::workspace) fn save_field_rename(&mut self, request: FieldRenameRequest) {
        if self.editing.has_save()
            || !self.state.can_review_field_rename()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.field_rename_request_is_current(&request)
            || !self.state.can_rebind_field_rename_layout(&request)
        {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        let ObjectKind::Table { name } = &request.target.tab.target.kind else {
            return;
        };
        let Some(filters) = self.capture_filter_restore_definitions() else {
            return;
        };
        let Some(request_id) = request_read(
            Command::ReviewFieldRename {
                document_revision: request.target.tab.target.document_revision,
                // Runtime field-rename command and retained FieldSaveHost request each own the
                // source table identity through asynchronous review.
                table: name.clone(),
                // Runtime field-rename command and retained FieldSaveHost exact request each
                // own the original native column name.
                column: request.target.column.clone(),
                native_number: request.target.native_number,
                // Runtime command and retained FieldSaveHost request each own the proposed name
                // for independent review/reopen validation.
                new_name: request.new_name.clone(),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_field_rename_review(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.field_rename = Some(FieldRenameHost {
            request_id,
            request,
            identity: None,
            witness: None,
            terminal: None,
            cleanup: None,
            filters,
            reopening: false,
            cancelled: false,
        });
        self.generation = request_id;
        self.pending = PendingOperation::FieldRenameReview { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_field_rename(
        &mut self,
        generation: u64,
        review: FieldRenameReview,
        capability: ReviewedFieldRename,
    ) {
        let Some(host) = &self.editing.field_rename else {
            return;
        };
        if generation != host.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::FieldRenameReview { request_id } if request_id == generation)
        {
            return;
        }
        if !self
            .state
            .field_rename_request_current(&host.request, generation)
            || !self.state.can_rebind_field_rename_layout(&host.request)
        {
            self.field_rename_review_failed(
                generation,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let witness = capability.reopen_witness();
        let target = &host.request.target;
        let matches = review.document_revision == target.tab.target.document_revision
            && matches!(&target.tab.target.kind, ObjectKind::Table { name } if *name == review.table)
            && review.column == target.column
            && review.native_number == target.native_number
            && review.new_name == host.request.new_name
            && review.changed == capability.changed()
            && capability.captured_table() == review.table
            && capability.captured_column() == review.column
            && capability.captured_native_number() == review.native_number
            && capability.captured_new_name() == review.new_name
            && witness.document_revision() == review.document_revision
            && witness.table_name() == review.table
            && witness.original_column_name() == review.column
            && witness.native_number() == review.native_number
            && witness.new_name() == review.new_name
            && witness.changed() == review.changed
            && capability.source_path() == witness.source_path();
        if !matches
            || !self
                .state
                .accept_field_rename_review(&host.request, generation, review.id)
        {
            self.field_rename_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        if !review.changed {
            self.start_unchanged_field_rename(generation, review.id, capability);
            return;
        }
        let host = self.editing.field_rename.as_ref().unwrap();
        let path = capability.source_path().to_path_buf();
        let Some(prepared) =
            self.state
                .prepare_field_rename_save(&host.request, generation, review.id)
        else {
            self.state
                .release_field_rename_accepted_review(&host.request, generation, review.id);
            self.field_rename_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        match self
            .editing
            .worker
            .start_field_rename(capability, journal_path(&path))
        {
            Ok(save_id) => {
                let identity = prepared.bind(save_id);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::FieldRename,
                    id: save_id,
                    path,
                    table: review.table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                let host = self.editing.field_rename.as_mut().unwrap();
                host.identity = Some(identity);
                host.witness = Some(witness);
                self.state.busy = true;
            }
            Err(error) => {
                drop(prepared);
                let message = error.to_string();
                drop(error.into_reviewed());
                self.field_rename_review_failed(generation, message);
            }
        }
    }

    fn start_unchanged_field_rename(
        &mut self,
        request_id: u64,
        review_id: u64,
        capability: ReviewedFieldRename,
    ) {
        let host = self.editing.field_rename.as_ref().unwrap();
        let Some(prepared) =
            self.state
                .prepare_unchanged_field_rename(&host.request, request_id, review_id)
        else {
            self.state
                .release_field_rename_accepted_review(&host.request, request_id, review_id);
            self.field_rename_review_failed(
                request_id,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        let witness = match capability.into_unchanged_reopen() {
            Ok(witness) => witness,
            Err(capability) => {
                drop(prepared);
                drop(capability);
                self.field_rename_review_failed(
                    request_id,
                    self.state.localizer.text("objects-open-refused"),
                );
                return;
            }
        };
        let identity = prepared.bind();
        let host = self.editing.field_rename.as_mut().unwrap();
        host.identity = Some(identity);
        // Unchanged rename host keeps a complete-image witness for retry while the explicit
        // read command independently owns that same immutable witness.
        host.witness = Some(std::sync::Arc::clone(&witness));
        host.reopening = true;
        let Some(generation) = self.issue_field_witness_read(FieldReadWitness::Rename(witness))
        else {
            return;
        };
        self.generation = generation;
        self.pending = PendingOperation::FieldRenameReopen { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn field_rename_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_rename
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.state
            .release_field_rename_review(&host.request, request_id);
        self.editing.field_rename = None;
        self.state.error = Some(message);
        self.state.busy = false;
        self.restore_field_rename_close_after_refusal();
    }

    pub(in crate::workspace) fn cancel_field_rename_review(&mut self, request_id: u64) {
        let Some(host) = self
            .editing
            .field_rename
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.worker.cancel();
        self.state
            .release_field_rename_review(&host.request, request_id);
        self.editing.field_rename = None;
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        self.restore_field_rename_close_after_refusal();
    }
}
