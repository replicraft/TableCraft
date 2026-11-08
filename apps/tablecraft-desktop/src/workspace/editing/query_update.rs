//! Route inspected UPDATE reviews and consume only runtime-issued save authority.
use super::{ActiveSave, Desktop, PendingOperation, SaveOwner};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, InspectedQuery, QueryUpdateReview as RuntimeReview, ReviewedQueryUpdate, journal_path,
};
use tablecraft_ui::QueryUpdateReview;

impl Desktop {
    pub(in crate::workspace) fn begin_query_update_from_inspection(
        &mut self,
        inspection_id: u64,
        target: Arc<InspectedQuery>,
    ) {
        if self.editing.has_save()
            || !self
                .state
                .query_parameter_inspection_current(inspection_id, target.sql())
        {
            return;
        }
        self.editing.clear_review();
        let request_id = self.worker.request(Command::ReviewQueryUpdate {
            target: Arc::clone(&target),
        });
        self.generation = request_id;
        if !self
            .state
            .begin_query_update_review(inspection_id, target.sql(), request_id)
        {
            self.worker.cancel();
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            self.state.busy = false;
            return;
        }
        self.query_parameters = None;
        self.editing.query_update_pending = Some((request_id, target));
        self.pending = PendingOperation::QueryUpdateReview { request_id };
        self.state.busy = true;
        self.state.error = None;
    }

    pub(in crate::workspace) fn release_stale_query_update_review(&mut self) {
        if self.editing.has_save() {
            return;
        }
        let Some((request_id, target)) = &self.editing.query_update_pending else {
            return;
        };
        if self
            .state
            .query_update_review_current(*request_id, target.sql())
        {
            return;
        }
        let request_id = *request_id;
        self.editing.query_update_pending = None;
        self.editing.query_update_review = None;
        if matches!(self.pending, PendingOperation::QueryUpdateReview { request_id: current } if current == request_id)
            && self.generation == request_id
        {
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn publish_query_update_review(
        &mut self,
        request_id: u64,
        review: RuntimeReview,
        capability: ReviewedQueryUpdate,
    ) {
        let current =
            self.editing
                .query_update_pending
                .as_ref()
                .is_some_and(|(expected, target)| {
                    *expected == request_id
                        && self
                            .state
                            .query_update_review_current(request_id, target.sql())
                });
        if !current
            || !matches!(self.pending, PendingOperation::QueryUpdateReview { request_id: expected } if expected == request_id)
        {
            return;
        }
        let id = review.id;
        if self.state.set_query_update_review(
            request_id,
            QueryUpdateReview {
                id,
                document_revision: review.document_revision,
                table: review.table,
                matched_rows: review.matched_rows,
                changed_rows: review.changed_rows,
            },
        ) {
            self.editing.query_update_review = Some((id, capability));
        } else {
            self.release_stale_query_update_review();
        }
    }

    pub(in crate::workspace) fn fail_query_update_review(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        if self.editing.query_update_review.is_some()
            || !self
                .editing
                .query_update_pending
                .as_ref()
                .is_some_and(|(expected, target)| {
                    *expected == request_id
                        && self
                            .state
                            .query_update_review_current(request_id, target.sql())
                })
            || !matches!(self.pending, PendingOperation::QueryUpdateReview { request_id: expected } if expected == request_id)
        {
            return;
        }
        self.state.fail_query_update_review(request_id, message);
        self.editing.query_update_pending = None;
        self.editing.query_update_review = None;
        self.pending = PendingOperation::Read;
    }

    pub(in crate::workspace) fn cancel_query_update_review(&mut self, request_id: u64) {
        if self.editing.has_save()
            || !self
                .editing
                .query_update_pending
                .as_ref()
                .is_some_and(|(expected, _)| *expected == request_id)
        {
            return;
        }
        self.state.close_query_update();
        self.editing.query_update_pending = None;
        self.editing.query_update_review = None;
        if matches!(self.pending, PendingOperation::QueryUpdateReview { request_id: expected } if expected == request_id)
            && self.generation == request_id
        {
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn apply_query_update(&mut self, review_id: u64) {
        if self.editing.has_save() || !self.state.can_apply_query_update(review_id) {
            return;
        }
        let Some((id, capability)) = self.editing.query_update_review.take() else {
            return;
        };
        let source_matches =
            self.state.database.as_ref().is_some_and(|database| {
                database.document_revision == capability.document_revision()
            });
        if id != review_id || !source_matches {
            self.editing.query_update_review = Some((id, capability));
            return;
        }
        let path = capability.source_path().to_path_buf();
        let table = capability.table().to_owned();
        match self
            .editing
            .worker
            .start_query_update(capability, journal_path(&path))
        {
            Ok(save_id) => {
                self.editing.query_update_pending = None;
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::QueryUpdate,
                    id: save_id,
                    path,
                    table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                self.state.begin_query_update_save(review_id, save_id);
                self.pending = PendingOperation::Read;
                self.state.busy = false;
            }
            Err(error) => {
                let message = error.to_string();
                self.editing.query_update_review =
                    error.into_reviewed().map(|capability| (id, capability));
                self.state.fail_query_update_apply(review_id, message);
            }
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
