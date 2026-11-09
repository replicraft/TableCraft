//! Exact destructive review composition; native admission and UI phases stay owned.
mod completion;
mod inspection;

use super::{ActiveSave, SaveOwner, field_save::FieldSaveHost};
use crate::workspace::{Desktop, PendingOperation, request_read};
use tablecraft_runtime::{
    Command, FieldDropReview, FieldDroppedReopen, ReviewedFieldDrop, journal_path,
};
use tablecraft_ui::{FieldDropRequest, FieldDropSaveIdentity, ObjectKind};

pub(in crate::workspace) type FieldDropHost =
    FieldSaveHost<FieldDropRequest, FieldDroppedReopen, FieldDropSaveIdentity>;

impl Desktop {
    pub(in crate::workspace) fn save_field_drop(&mut self, request: FieldDropRequest) {
        if self.editing.has_save()
            || self.state.native_source_stale()
            || !self.state.can_review_field_drop()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.field_drop_request_is_current(&request)
            || !self.state.can_rebind_field_drop_layout(&request)
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
            Command::ReviewFieldDrop {
                document_revision: request.target.tab.target.document_revision,
                table: name.clone(),
                column: request.target.column.clone(),
                native_number: request.target.native_number,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_field_drop_review(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.field_drop = Some(FieldDropHost {
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
        self.pending = PendingOperation::FieldDropReview { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_field_drop(
        &mut self,
        generation: u64,
        review: FieldDropReview,
        capability: ReviewedFieldDrop,
    ) {
        let Some(host) = &self.editing.field_drop else {
            return;
        };
        if generation != host.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::FieldDropReview { request_id } if request_id == generation)
        {
            return;
        }
        if !self
            .state
            .field_drop_request_current(&host.request, generation)
            || !self.state.can_rebind_field_drop_layout(&host.request)
        {
            self.field_drop_review_failed(
                generation,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        let witness = capability.reopen_witness();
        let target = &host.request.target;
        let exact = review.document_revision == target.tab.target.document_revision
            && matches!(&target.tab.target.kind, ObjectKind::Table { name } if *name == review.table)
            && review.column == target.column
            && review.native_number == target.native_number
            && capability.captured_table() == review.table
            && capability.captured_column() == review.column
            && capability.captured_native_number() == review.native_number
            && witness.document_revision() == review.document_revision
            && witness.table_name() == review.table
            && witness.original_column_name() == review.column
            && witness.native_number() == review.native_number
            && capability.source_path() == witness.source_path();
        if !exact
            || !self
                .state
                .accept_field_drop_review(&host.request, generation, review.id)
        {
            self.field_drop_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let host = self.editing.field_drop.as_ref().unwrap();
        let path = capability.source_path().to_path_buf();
        let Some(prepared) =
            self.state
                .prepare_field_drop_save(&host.request, generation, review.id)
        else {
            self.state
                .release_field_drop_accepted_review(&host.request, generation, review.id);
            self.field_drop_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        match self
            .editing
            .worker
            .start_field_drop(capability, journal_path(&path))
        {
            Ok(save_id) => {
                let identity = prepared.bind(save_id);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::FieldDrop,
                    id: save_id,
                    path,
                    table: review.table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                let host = self.editing.field_drop.as_mut().unwrap();
                host.identity = Some(identity);
                host.witness = Some(witness);
                self.state.busy = true;
            }
            Err(error) => {
                drop(prepared);
                let message = error.to_string();
                drop(error.into_reviewed());
                self.field_drop_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn field_drop_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_drop
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.state
            .release_field_drop_review(&host.request, request_id);
        self.editing.field_drop = None;
        self.state.error = Some(message);
        if self.generation == request_id
            && matches!(self.pending, PendingOperation::FieldDropReview { request_id: current } if current == request_id)
        {
            self.pending = PendingOperation::Read;
            self.state.busy = false;
        }
        self.restore_field_drop_close_after_refusal();
    }

    pub(in crate::workspace) fn cancel_field_drop_review(&mut self, request_id: u64) {
        let Some(host) = self
            .editing
            .field_drop
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        if self.worker.publication_is_current(request_id) {
            self.worker.cancel();
        }
        self.state
            .release_field_drop_review(&host.request, request_id);
        self.editing.field_drop = None;
        if self.generation == request_id
            && matches!(self.pending, PendingOperation::FieldDropReview { request_id: current } if current == request_id)
        {
            self.pending = PendingOperation::Read;
            self.state.busy = false;
        }
        self.restore_field_drop_close_after_refusal();
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
