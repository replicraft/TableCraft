//! Desktop composes real runtime capabilities; shared UI owns confirmation state.
mod completion;
mod inspection;
#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;

use super::{ActiveSave, SaveOwner, field_save::FieldSaveHost};
use crate::workspace::{Desktop, PendingOperation, request_read};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, ReviewedTableDrop, TableDropReview, TableDroppedReopen, journal_path,
};
use tablecraft_ui::{TableDropRequest, TableDropSaveIdentity};

pub(in crate::workspace) struct TableDropHost {
    pub(super) save:
        FieldSaveHost<Arc<TableDropRequest>, TableDroppedReopen, TableDropSaveIdentity>,
    native_id: i32,
    declared_rows: u32,
}

pub(in crate::workspace) struct TableDropObservation {
    request_id: u64,
    request: Arc<TableDropRequest>,
    facts: Option<(i32, u32)>,
}

impl Desktop {
    pub(in crate::workspace) fn save_table_drop(&mut self, request: Arc<TableDropRequest>) {
        if self.editing.has_save()
            || self.state.native_source_stale()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.can_rebind_table_drop_layout(&request)
        {
            self.state.reject_table_drop_review(
                &request,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let Some(observation) = self
            .editing
            .table_drop_observation
            .as_ref()
            .filter(|observation| observation.request == request)
        else {
            self.state.reject_table_drop_review(
                &request,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        let Some((native_id, declared_rows)) = observation.facts else {
            self.state.reject_table_drop_review(
                &request,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        let Some(table) = request.table_name() else {
            return;
        };
        let Some(filters) = self.capture_filter_restore_definitions() else {
            self.state.reject_table_drop_review(
                &request,
                self.state.localizer.text("objects-restore-refused"),
            );
            return;
        };
        // NO_CLONE: The runtime command owns the native table name across its worker boundary;
        // the immutable Arc request remains shared by UI/host for later exact callbacks.
        let command = Command::ReviewTableDrop {
            document_revision: request.target.document_revision,
            table: table.to_owned(),
        };
        let Some(request_id) = request_read(command, &self.worker, &mut self.state) else {
            self.state.reject_table_drop_review(
                &request,
                self.state
                    .localizer
                    .text("objects-read-identities-exhausted"),
            );
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_table_drop_review(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.table_drop_observation = None;
        self.editing.table_drop = Some(TableDropHost {
            save: FieldSaveHost {
                request_id,
                request,
                identity: None,
                witness: None,
                terminal: None,
                cleanup: None,
                filters,
                reopening: false,
                cancelled: false,
            },
            native_id,
            declared_rows,
        });
        self.generation = request_id;
        self.pending = PendingOperation::TableDropReview { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_table_drop(
        &mut self,
        generation: u64,
        review: TableDropReview,
        capability: ReviewedTableDrop,
    ) {
        let Some(host) = &self.editing.table_drop else {
            return;
        };
        if generation != host.save.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::TableDropReview { request_id } if request_id == generation)
        {
            return;
        }
        let witness = capability.reopen_witness();
        let exact = self
            .state
            .table_drop_review_matches(&host.save.request, generation)
            && self.state.can_rebind_table_drop_layout(&host.save.request)
            && host.save.request.target.document_revision == review.document_revision
            && host.save.request.table_name() == Some(review.table.as_str())
            && review.native_id == host.native_id
            && review.declared_rows == host.declared_rows
            && capability.captured_table() == review.table
            && capability.captured_native_id() == review.native_id
            && capability.captured_document_revision() == review.document_revision
            && witness.document_revision() == review.document_revision
            && witness.table_name() == review.table
            && witness.native_id() == review.native_id
            && capability.source_path() == witness.source_path();
        if !exact || review.id == 0 {
            self.table_drop_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let path = capability.source_path().to_path_buf();
        match self
            .editing
            .worker
            .start_table_drop(capability, journal_path(&path))
        {
            Ok(save_id) => {
                let identity = TableDropSaveIdentity {
                    request_id: generation,
                    review_id: review.id,
                    save_id,
                };
                let Some(host) = self.editing.table_drop.as_mut() else {
                    return;
                };
                if !self
                    .state
                    .begin_table_drop_save(&host.save.request, identity)
                {
                    self.editing.worker.cancel(save_id);
                    self.state.mark_native_save_uncertain();
                }
                host.save.identity = Some(identity);
                host.save.witness = Some(witness);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::TableDrop,
                    id: save_id,
                    path,
                    table: review.table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                self.state.busy = true;
            }
            Err(error) => {
                let message = error.to_string();
                drop(error.into_reviewed());
                self.table_drop_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn table_drop_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .table_drop
            .as_ref()
            .filter(|host| host.save.request_id == request_id && host.save.identity.is_none())
        else {
            return;
        };
        self.state
            .fail_table_drop_review(&host.save.request, request_id, message);
        self.editing.table_drop = None;
        if self.generation == request_id {
            self.pending = PendingOperation::Read;
            self.state.busy = false;
        }
    }

    pub(in crate::workspace) fn cancel_table_drop_review(&mut self, request_id: u64) {
        let Some(host) = self
            .editing
            .table_drop
            .as_ref()
            .filter(|host| host.save.request_id == request_id && host.save.identity.is_none())
        else {
            return;
        };
        if self.worker.publication_is_current(request_id) {
            self.worker.cancel();
        }
        self.state
            .cancel_table_drop_review(&host.save.request, request_id);
        self.editing.table_drop = None;
        if self.generation == request_id {
            self.pending = PendingOperation::Read;
            self.state.busy = false;
        }
    }
}
