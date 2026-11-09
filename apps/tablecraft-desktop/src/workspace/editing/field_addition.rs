//! Thin host ownership of native field-addition review, save and full-witness reopening.
mod abandon;
mod completion;
mod inspection_completion;

use super::{ActiveSave, SaveOwner};
use crate::workspace::{Desktop, PendingOperation, filtering::RestoreFilter};
use std::sync::Arc;
use tablecraft_runtime::{
    AddedColumnReopen, Command, FieldAdditionAvailability, FieldAdditionReview,
    ReviewedFieldAddition, SaveOutcome, journal_path,
};
use tablecraft_ui::{
    FieldAdditionRequest, FieldAdditionSaveIdentity, ObjectKind, ObjectTab, SaveCleanupNotice,
};

pub(in crate::workspace) struct FieldAdditionHost {
    request_id: u64,
    request: FieldAdditionRequest,
    pub(super) identity: Option<FieldAdditionSaveIdentity>,
    witness: Option<Arc<AddedColumnReopen>>,
    terminal: Option<SaveOutcome>,
    cleanup: Option<SaveCleanupNotice>,
    filters: Vec<RestoreFilter>,
    reopening: bool,
    cancelled: bool,
}

impl Desktop {
    /// Invoked after a table page is published/activated. The UI cache suppresses
    /// all repeat automatic proof, including prior refusals/errors.
    pub(in crate::workspace) fn inspect_field_addition_availability(&mut self) {
        self.state.prune_field_addition_inspections();
        let Some(tab) = self.state.active_object() else {
            return;
        };
        if self.editing.has_save() || !self.state.needs_field_addition_inspection(&tab) {
            return;
        }
        let ObjectKind::Table { name } = &tab.target.kind else {
            return;
        };
        let command = Command::InspectFieldAdditionAvailability {
            document_revision: tab.target.document_revision,
            table: name.clone(),
        };
        if self.state.prepare_field_addition_inspection(&tab).is_err() {
            return;
        }
        let request_id = self.worker.request(command);
        if !self.state.bind_field_addition_inspection(&tab, request_id) {
            self.worker.cancel();
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::FieldAdditionAvailability { request_id, tab };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_field_addition_availability(
        &mut self,
        generation: u64,
        availability: FieldAdditionAvailability,
    ) {
        let PendingOperation::FieldAdditionAvailability { request_id, tab } = &self.pending else {
            return;
        };
        if generation != *request_id
            || availability.document_revision != tab.target.document_revision
            || !matches!(&tab.target.kind, ObjectKind::Table { name } if *name == availability.table)
        {
            return;
        }
        self.state.set_field_addition_availability(
            tab,
            *request_id,
            availability.number_available,
            availability.short_text_available,
        );
    }

    pub(in crate::workspace) fn field_addition_availability_failed(
        &mut self,
        generation: u64,
        message: String,
    ) {
        let PendingOperation::FieldAdditionAvailability { request_id, tab } = &self.pending else {
            return;
        };
        if generation == *request_id {
            self.state
                .fail_field_addition_inspection(tab, *request_id, message);
        }
    }

    pub(in crate::workspace) fn retry_field_addition_availability(&mut self, tab: ObjectTab) {
        if !self.editing.has_save() && self.state.retry_field_addition_inspection(&tab) {
            self.inspect_field_addition_availability();
        }
    }

    pub(in crate::workspace) fn save_field_addition(&mut self, request: FieldAdditionRequest) {
        if self.editing.has_save()
            || self.state.field_addition_source_stale()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.field_addition_request_is_current(&request)
            || !self.state.can_rebind_field_addition_layout(&request)
        {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        let ObjectKind::Table { name } = &request.tab.target.kind else {
            return;
        };
        let Some(filters) = self.capture_filter_restore_definitions() else {
            return;
        };
        let request_id = self.worker.request(Command::ReviewAddField {
            document_revision: request.tab.target.document_revision,
            table: name.clone(),
            column: request.column.clone(),
            declaration: request.declaration,
        });
        if !self.state.begin_field_addition_review(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.field_addition = Some(FieldAdditionHost {
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
        self.pending = PendingOperation::FieldAdditionReview { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_field_addition(
        &mut self,
        generation: u64,
        review: FieldAdditionReview,
        capability: ReviewedFieldAddition,
    ) {
        let Some(host) = &self.editing.field_addition else {
            return;
        };
        if generation != host.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::FieldAdditionReview { request_id } if request_id == generation)
        {
            return;
        }
        if !self
            .state
            .field_addition_request_current(&host.request, generation)
            || !self.state.can_rebind_field_addition_layout(&host.request)
        {
            self.field_addition_review_failed(
                generation,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        }
        if host.cancelled {
            self.state
                .release_field_addition_review(&host.request, generation);
            self.editing.field_addition = None;
            self.state.busy = false;
            self.restore_field_addition_close_after_refusal();
            return;
        }
        let witness = capability.reopen_witness();
        let matches = review.declaration == host.request.declaration
            && capability.captured_declaration() == host.request.declaration
            && review.document_revision == host.request.tab.target.document_revision
            && matches!(&host.request.tab.target.kind, ObjectKind::Table { name } if *name == review.table)
            && review.column == host.request.column
            && witness.document_revision() == review.document_revision
            && witness.table_name() == review.table
            && witness.column_name() == review.column
            && capability.source_path() == witness.source_path();
        if !matches {
            self.field_addition_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        if !self
            .state
            .accept_field_addition_review(&host.request, generation, review.id)
        {
            self.field_addition_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let path = capability.source_path().to_path_buf();
        let Some(prepared) =
            self.state
                .prepare_field_addition_save(&host.request, generation, review.id)
        else {
            self.state
                .release_field_addition_accepted_review(&host.request, generation, review.id);
            self.field_addition_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        match self
            .editing
            .worker
            .start_field_addition(capability, journal_path(&path))
        {
            Ok(save_id) => {
                let identity = prepared.bind(save_id);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::FieldAddition,
                    id: save_id,
                    path,
                    table: review.table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                let host = self.editing.field_addition.as_mut().unwrap();
                host.identity = Some(identity);
                host.witness = Some(witness);
                self.state.busy = true;
            }
            Err(error) => {
                drop(prepared);
                let message = error.to_string();
                drop(error.into_reviewed());
                self.field_addition_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn field_addition_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .field_addition
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.state
            .release_field_addition_review(&host.request, request_id);
        self.editing.field_addition = None;
        self.state.error = Some(message);
        self.state.busy = false;
        self.restore_field_addition_close_after_refusal();
    }

    pub(in crate::workspace) fn cancel_field_addition_review(&mut self, request_id: u64) {
        let Some(host) = self
            .editing
            .field_addition
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        // Worker cancellation revokes reply publication, including an already
        // completed capability. This readonly review has no native save actor;
        // no terminal callback can release its presentation after cancellation.
        self.worker.cancel();
        self.state
            .release_field_addition_review(&host.request, request_id);
        self.editing.field_addition = None;
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        self.restore_field_addition_close_after_refusal();
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
