//! Shared authoring identity and real Runtime capability checks before saving.
use super::super::{ActiveSave, SaveOwner};
use super::QueryCreationHost;
use crate::workspace::{Desktop, PendingOperation, request_read};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, QueryCreationOrigin, QueryCreationReview, QueryCreationTarget, ReviewedQueryCreation,
    journal_path,
};
use tablecraft_ui::QueryCreationRequest;

impl Desktop {
    pub(in crate::workspace) fn save_query_creation(&mut self, request: Arc<QueryCreationRequest>) {
        if self.editing.has_save()
            || self.state.native_mutation_blocked()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.query_creation_request_current(&request)
            || !self.state.can_rebind_query_creation_layout(&request)
        {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        let Some(filters) = self.capture_filter_restore_definitions() else {
            return;
        };
        // NO_CLONE: Runtime owns the command's authored SQL and source target on
        // its worker thread. The shared request remains the immutable publication
        // precondition until full-image reopen; those independent owners outlive
        // this call and neither accepts a borrowed UI frame.
        let source = request
            .source
            .as_ref()
            .map(|target| tablecraft_runtime::SavedQueryTarget {
                document_revision: target.document_revision,
                object_id: target.object_id,
                name: target.name.clone(),
            });
        // NO_CLONE: Runtime and the shared request independently retain the same
        // immutable submitted SQL across worker review and guarded publication.
        let sql = Arc::clone(&request.sql);
        // NO_CLONE: Runtime consumes the destination while the shared request
        // retains the independently issued naming/publication precondition.
        let destination = request.destination.clone();
        let Some(request_id) = request_read(
            Command::ReviewQueryCreation {
                target: QueryCreationTarget {
                    document_revision: request.document_revision,
                    draft_revision: request.text_revision,
                    source,
                },
                sql,
                destination,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        // NO_CLONE: Shared UI retains the same immutable request while the host
        // owns reply correlation and eventual native terminal/reopen handling.
        if !self
            .state
            .begin_query_creation_review(request_id, Arc::clone(&request))
        {
            self.worker.cancel();
            return;
        }
        self.editing.query_creation = Some(QueryCreationHost {
            request_id,
            request,
            review_id: None,
            witness: None,
            status: None,
            terminal: None,
            cleanup: None,
            filters,
            reopening: false,
            cancellation_requested: false,
        });
        self.generation = request_id;
        self.pending = PendingOperation::QueryCreationReview { request_id };
        self.state.busy = true;
        self.state.error = None;
    }

    pub(in crate::workspace) fn start_reviewed_query_creation(
        &mut self,
        generation: u64,
        review: QueryCreationReview,
        capability: ReviewedQueryCreation,
    ) {
        let Some(host) = &self.editing.query_creation else {
            return;
        };
        if generation != self.generation
            || generation != host.request_id
            || !matches!(self.pending, PendingOperation::QueryCreationReview { request_id } if request_id == generation)
        {
            return;
        }
        let request = &host.request;
        let origin_matches = match &review.origin {
            QueryCreationOrigin::Draft {
                draft_revision,
                sql,
                source,
            } => {
                *draft_revision == request.text_revision
                    && sql.as_str() == request.sql.as_str()
                    && match (source, &request.source) {
                        (None, None) => true,
                        (Some(actual), Some(expected)) => {
                            actual.document_revision == expected.document_revision
                                && actual.object_id == expected.object_id
                                && actual.name == expected.name
                        }
                        _ => false,
                    }
            }
            QueryCreationOrigin::Saved(_) => false,
        };
        let witness = capability.reopen_witness();
        let exact = origin_matches
            && review.document_revision == request.document_revision
            && review.destination == request.destination
            && capability.id() == review.id
            && witness.document_revision() == request.document_revision
            && witness.query_name() == request.destination
            && witness.native_query_id() == review.native_id
            && witness.source_path() == capability.source_path()
            && self.state.query_creation_request_current(request);
        if !exact
            || !self
                .state
                .set_query_creation_review(generation, request, review.id)
        {
            self.query_creation_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let Some(host) = self.editing.query_creation.as_mut() else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        host.review_id = Some(review.id);
        host.witness = Some(witness);
        let Some(prepared) =
            self.state
                .prepare_query_creation_save(generation, &host.request, review.id)
        else {
            self.query_creation_review_failed(
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
            .start_query_creation(capability, journal_path(&path))
        {
            Ok(id) => {
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::QueryCreation,
                    id,
                    path,
                    table: String::new(),
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                if let Some(status) = self.state.bind_query_creation_save(prepared, id) {
                    if let Some(host) = self.editing.query_creation.as_mut() {
                        host.status = Some(status);
                    } else {
                        self.editing.worker.cancel(id);
                        self.state.error = Some(self.state.localizer.text("objects-open-refused"));
                    }
                } else {
                    // Accepted native ownership survives even a presentation
                    // binding failure; its real terminal report must be retained.
                    self.editing.worker.cancel(id);
                    self.state.error = Some(self.state.localizer.text("objects-open-refused"));
                }
                self.state.busy = true;
            }
            Err(error) => {
                self.state.abort_query_creation_save(prepared);
                if let Some(host) = self.editing.query_creation.as_mut() {
                    host.status = None;
                }
                let message = error.to_string();
                drop(error.into_reviewed());
                self.query_creation_review_failed(generation, message);
            }
        }
    }

    pub(in crate::workspace) fn query_creation_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .query_creation
            .as_ref()
            .filter(|host| host.request_id == request_id)
        else {
            return;
        };
        let mut accepted = if let Some(review_id) = host.review_id {
            self.state.release_query_creation_review(
                request_id,
                &host.request,
                review_id,
                Some(message),
            )
        } else {
            self.state
                .fail_query_creation(request_id, &host.request, message)
        };
        // Error presentation can refuse its own paid text without revoking the
        // independently retained request. Retire that exact owner without a new
        // payload before reporting the host's existing resource message.
        if !accepted {
            accepted = if let Some(review_id) = host.review_id {
                self.state
                    .release_query_creation_review(request_id, &host.request, review_id, None)
            } else {
                self.state
                    .fail_query_creation(request_id, &host.request, String::new())
            };
            if accepted {
                self.state.error = Some(self.state.localizer.text("objects-memory-limit"));
            }
        }
        if accepted {
            if let Some(status) = host.status {
                self.state.clear_query_creation_save(status);
            }
            self.editing.query_creation = None;
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn cancel_query_creation_operation(&mut self) -> bool {
        let Some(host) = &self.editing.query_creation else {
            return false;
        };
        if self.editing.save.is_some() {
            return false;
        }
        if host.review_id.is_none() {
            let id = host.request_id;
            self.worker.cancel();
            self.query_creation_review_failed(
                id,
                self.state.localizer.text("shell-operation-cancelled"),
            );
        } else if host.reopening {
            self.worker.cancel();
            self.fail_query_creation_reopen(&tablecraft_runtime::Error::Cancelled);
        }
        true
    }
}
