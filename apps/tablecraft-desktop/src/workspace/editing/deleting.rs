//! Bridge a source-bound deletion review into the shared native save owner.
use super::{ActiveSave, Desktop, PendingOperation, SaveOwner};
use tablecraft_runtime::{
    Command, DeleteReview as RuntimeReview, DeleteTarget as RuntimeTarget, ReviewedDelete,
    journal_path,
};
use tablecraft_ui::{DeleteReview, DeleteTarget};

impl Desktop {
    pub(in crate::workspace) fn review_delete(&mut self, target: DeleteTarget) {
        if self.editing.has_save() || !self.state.can_delete_row(&target) {
            return;
        }
        self.editing.clear_review();
        let request_id = self.worker.request(Command::ReviewDelete {
            target: RuntimeTarget {
                document_revision: target.document_revision,
                table: target.table.clone(),
                parent_ordinal: target.parent_ordinal,
            },
        });
        self.generation = request_id;
        self.pending = PendingOperation::Delete { request_id };
        self.state.begin_delete(target, request_id);
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_delete_review(
        &mut self,
        request_id: u64,
        review: RuntimeReview,
        capability: ReviewedDelete,
    ) {
        let id = review.id;
        if self.state.set_delete_review(
            request_id,
            DeleteReview {
                id,
                target: DeleteTarget {
                    document_revision: review.target.document_revision,
                    table: review.target.table,
                    parent_ordinal: review.target.parent_ordinal,
                },
                columns: crate::view_models::columns(review.columns),
                row: review
                    .row
                    .into_iter()
                    .map(crate::view_models::cell)
                    .collect(),
                deleted_rows: review.deleted_rows,
            },
        ) {
            self.editing.delete_review = Some((id, capability));
        }
    }

    pub(in crate::workspace) fn close_delete(&mut self, request_id: u64) {
        if self.editing.has_save() {
            return;
        }
        if matches!(self.pending, PendingOperation::Delete { request_id: active } if active == request_id)
            && self.generation == request_id
        {
            self.editing.clear_review();
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
            self.state.close_delete();
        }
    }

    pub(in crate::workspace) fn apply_delete(&mut self, review_id: u64) {
        if self.editing.has_save() || !self.state.can_apply_delete(review_id) {
            return;
        }
        let Some((id, capability)) = self.editing.delete_review.take() else {
            return;
        };
        let source_matches = self.state.database.as_ref().is_some_and(|database| {
            database.document_revision == capability.target().document_revision
        }) && self.state.data.as_ref().is_some_and(|data| {
            data.source_revision == Some(capability.target().document_revision)
                && data.table == capability.target().table
        });
        if id != review_id || !source_matches {
            self.editing.delete_review = Some((id, capability));
            return;
        }
        let path = capability.source_path().to_path_buf();
        let table = capability.target().table.clone();
        let offset = capability.refresh_ordinal();
        match self
            .editing
            .worker
            .start_delete(capability, journal_path(&path))
        {
            Ok(save_id) => {
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::Delete,
                    id: save_id,
                    path,
                    table,
                    offset,
                    report: None,
                });
                self.state.begin_delete_save(review_id, save_id);
                self.state.busy = false;
            }
            Err(error) => {
                let message = error.to_string();
                self.editing.delete_review =
                    error.into_reviewed().map(|capability| (id, capability));
                self.state.fail_delete_apply(review_id, message);
            }
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
