//! Thin host ownership of native Number review, save and full-witness reopening.
mod completion;

use super::{ActiveSave, SaveOwner};
use crate::workspace::{Desktop, PendingOperation, filtering::RestoreFilter};
use std::sync::Arc;
use tablecraft_runtime::{
    AddColumnReview, AddedColumnReopen, Command, NumberColumnAvailability, ReviewedNumberColumn,
    SaveOutcome, journal_path,
};
use tablecraft_ui::{
    NumberColumnRequest, NumberColumnSaveIdentity, ObjectKind, ObjectTab, SaveCleanupNotice,
};

pub(in crate::workspace) struct NumberColumnHost {
    request_id: u64,
    request: NumberColumnRequest,
    pub(super) identity: Option<NumberColumnSaveIdentity>,
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
    pub(in crate::workspace) fn inspect_number_column_availability(&mut self) {
        self.state.prune_number_column_inspections();
        let Some(tab) = self.state.active_object() else {
            return;
        };
        if self.editing.has_save() || !self.state.needs_number_column_inspection(&tab) {
            return;
        }
        let ObjectKind::Table { name } = &tab.target.kind else {
            return;
        };
        let command = Command::InspectNumberColumnAvailability {
            document_revision: tab.target.document_revision,
            table: name.clone(),
        };
        if self.state.prepare_number_column_inspection(&tab).is_err() {
            return;
        }
        let request_id = self.worker.request(command);
        if !self.state.bind_number_column_inspection(&tab, request_id) {
            self.worker.cancel();
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::NumberColumnAvailability { request_id, tab };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_number_column_availability(
        &mut self,
        generation: u64,
        availability: NumberColumnAvailability,
    ) {
        let PendingOperation::NumberColumnAvailability { request_id, tab } = &self.pending else {
            return;
        };
        if generation != *request_id
            || availability.document_revision != tab.target.document_revision
            || !matches!(&tab.target.kind, ObjectKind::Table { name } if *name == availability.table)
        {
            return;
        }
        self.state
            .set_number_column_availability(tab, *request_id, availability.available);
    }

    pub(in crate::workspace) fn number_column_availability_failed(
        &mut self,
        generation: u64,
        message: String,
    ) {
        let PendingOperation::NumberColumnAvailability { request_id, tab } = &self.pending else {
            return;
        };
        if generation == *request_id {
            self.state
                .fail_number_column_inspection(tab, *request_id, message);
        }
    }

    pub(in crate::workspace) fn retry_number_column_availability(&mut self, tab: ObjectTab) {
        if !self.editing.has_save() && self.state.retry_number_column_inspection(&tab) {
            self.inspect_number_column_availability();
        }
    }

    pub(in crate::workspace) fn save_number_column(&mut self, request: NumberColumnRequest) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.number_column_request_is_current(&request)
            || !self.state.can_rebind_number_column_layout(&request)
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
        let request_id = self.worker.request(Command::ReviewAddNumberColumn {
            document_revision: request.tab.target.document_revision,
            table: name.clone(),
            column: request.column.clone(),
        });
        if !self.state.begin_number_column_review(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.number_column = Some(NumberColumnHost {
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
        self.pending = PendingOperation::NumberColumnReview { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_number_column(
        &mut self,
        generation: u64,
        review: AddColumnReview,
        capability: ReviewedNumberColumn,
    ) {
        let Some(host) = &self.editing.number_column else {
            return;
        };
        if generation != host.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::NumberColumnReview { request_id } if request_id == generation)
            || !self
                .state
                .number_column_request_current(&host.request, generation)
            || !self.state.can_rebind_number_column_layout(&host.request)
        {
            return;
        }
        if host.cancelled {
            self.state
                .release_number_column_review(&host.request, generation);
            self.editing.number_column = None;
            self.state.busy = false;
            return;
        }
        let witness = capability.reopen_witness();
        let matches = review.document_revision == host.request.tab.target.document_revision
            && matches!(&host.request.tab.target.kind, ObjectKind::Table { name } if *name == review.table)
            && review.column == host.request.column
            && witness.document_revision() == review.document_revision
            && witness.table_name() == review.table
            && witness.column_name() == review.column
            && capability.source_path() == witness.source_path();
        if !matches {
            self.number_column_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        if !self
            .state
            .accept_number_column_review(&host.request, generation, review.id)
        {
            return;
        }
        let path = capability.source_path().to_path_buf();
        match self
            .editing
            .worker
            .start_number_column(capability, journal_path(&path))
        {
            Ok(save_id) => {
                let identity = NumberColumnSaveIdentity {
                    request_id: generation,
                    review_id: review.id,
                    save_id,
                };
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::NumberColumn,
                    id: save_id,
                    path,
                    table: review.table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                let host = self.editing.number_column.as_mut().unwrap();
                host.identity = Some(identity);
                host.witness = Some(witness);
                if !self.state.bind_number_column_save(
                    &host.request,
                    generation,
                    review.id,
                    save_id,
                ) || !self.state.begin_number_column_save_status(identity)
                {
                    // Retain the actual accepted worker and owner on any invariant
                    // failure; never drop a running actor or invent completion.
                    self.editing.worker.cancel(save_id);
                    self.state.error = Some(self.state.localizer.text("objects-open-refused"));
                }
                self.state.busy = true;
            }
            Err(error) => {
                let message = error.to_string();
                drop(error.into_reviewed());
                self.state.release_number_column_accepted_review(
                    &self.editing.number_column.as_ref().unwrap().request,
                    generation,
                    review.id,
                );
                self.number_column_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn number_column_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .number_column
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.state
            .release_number_column_review(&host.request, request_id);
        self.editing.number_column = None;
        self.state.error = Some(message);
        self.state.busy = false;
    }

    pub(in crate::workspace) fn cancel_number_column_review(&mut self, request_id: u64) {
        if let Some(host) = self
            .editing
            .number_column
            .as_mut()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        {
            host.cancelled = true;
            self.worker.cancel();
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
