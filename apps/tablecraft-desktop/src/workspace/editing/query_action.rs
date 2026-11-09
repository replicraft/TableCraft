//! Route typed SQL action reviews without parsing or executing SQL in the host.
use super::{ActiveSave, Desktop, PendingOperation, SaveOwner};
use crate::workspace::query_parameters::runtime_inputs;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tablecraft_runtime::{
    Command, InspectedQuery, QueryDeleteReview as RuntimeDeleteReview,
    QueryInsertReview as RuntimeInsertReview, QueryOperation,
    QueryUpdateReview as RuntimeUpdateReview, ReviewedQueryDelete, ReviewedQueryInsert,
    ReviewedQueryUpdate, SaveWorker, journal_path,
};
use tablecraft_ui::{ParameterInput, QueryActionEffect, QueryActionKind, QueryActionReview};

pub(super) enum ReviewedAction {
    Update(ReviewedQueryUpdate),
    Delete(ReviewedQueryDelete),
    Insert(ReviewedQueryInsert),
}

impl ReviewedAction {
    fn kind(&self) -> QueryActionKind {
        match self {
            Self::Update(_) => QueryActionKind::Update,
            Self::Delete(_) => QueryActionKind::Delete,
            Self::Insert(_) => QueryActionKind::Insert,
        }
    }

    fn document_revision(&self) -> u64 {
        match self {
            Self::Update(capability) => capability.document_revision(),
            Self::Delete(capability) => capability.document_revision(),
            Self::Insert(capability) => capability.document_revision(),
        }
    }

    fn source_path(&self) -> &Path {
        match self {
            Self::Update(capability) => capability.source_path(),
            Self::Delete(capability) => capability.source_path(),
            Self::Insert(capability) => capability.source_path(),
        }
    }

    fn table(&self) -> &str {
        match self {
            Self::Update(capability) => capability.table(),
            Self::Delete(capability) => capability.table(),
            Self::Insert(capability) => capability.table(),
        }
    }

    fn start(self, worker: &SaveWorker, bundle: PathBuf) -> Result<u64, (String, Option<Self>)> {
        match self {
            Self::Update(capability) => {
                worker
                    .start_query_update(capability, bundle)
                    .map_err(|error| {
                        let message = error.to_string();
                        (message, error.into_reviewed().map(Self::Update))
                    })
            }
            Self::Delete(capability) => {
                worker
                    .start_query_delete(capability, bundle)
                    .map_err(|error| {
                        let message = error.to_string();
                        (message, error.into_reviewed().map(Self::Delete))
                    })
            }
            Self::Insert(capability) => {
                worker
                    .start_query_insert(capability, bundle)
                    .map_err(|error| {
                        let message = error.to_string();
                        (message, error.into_reviewed().map(Self::Insert))
                    })
            }
        }
    }
}

fn action_kind(target: &InspectedQuery) -> Option<QueryActionKind> {
    match target.operation() {
        QueryOperation::Update => Some(QueryActionKind::Update),
        QueryOperation::Delete => Some(QueryActionKind::Delete),
        QueryOperation::Insert => Some(QueryActionKind::Insert),
        QueryOperation::Read | QueryOperation::Unsupported => None,
    }
}

impl Desktop {
    pub(in crate::workspace) fn begin_query_action_from_parameters(
        &mut self,
        inspection_id: u64,
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterInput>,
    ) {
        if self.editing.has_save()
            || !self
                .state
                .can_run_parameterized_query(inspection_id, target.sql(), &parameters)
        {
            return;
        }
        let Some(kind) = action_kind(&target) else {
            return;
        };
        if self.state.field_addition_source_stale() {
            return;
        }
        self.editing.clear_review();
        let inputs = runtime_inputs(&parameters);
        let command = match kind {
            QueryActionKind::Update => Command::ReviewQueryUpdate {
                target: Arc::clone(&target),
                parameters: inputs,
            },
            QueryActionKind::Delete => Command::ReviewQueryDelete {
                target: Arc::clone(&target),
                parameters: inputs,
            },
            QueryActionKind::Insert => Command::ReviewQueryInsert {
                target: Arc::clone(&target),
                parameters: inputs,
            },
        };
        let request_id = self.worker.request(command);
        self.generation = request_id;
        if !self.state.begin_query_action_review(
            inspection_id,
            target.sql(),
            request_id,
            parameters,
            kind,
        ) {
            self.worker.cancel();
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            self.state.busy = false;
            return;
        }
        self.query_parameters = None;
        self.editing.query_action_pending = Some((request_id, target));
        self.pending = PendingOperation::QueryActionReview { request_id };
        self.state.busy = true;
        self.state.error = None;
    }

    pub(in crate::workspace) fn release_stale_query_action_review(&mut self) {
        if self.editing.has_save() {
            return;
        }
        let Some((request_id, target)) = &self.editing.query_action_pending else {
            return;
        };
        if self
            .state
            .query_action_review_current(*request_id, target.sql())
        {
            return;
        }
        let request_id = *request_id;
        self.editing.query_action_pending = None;
        self.editing.query_action_review = None;
        if matches!(self.pending, PendingOperation::QueryActionReview { request_id: current } if current == request_id)
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
        review: RuntimeUpdateReview,
        capability: ReviewedQueryUpdate,
    ) {
        self.publish_query_action_review(
            request_id,
            QueryActionReview {
                id: review.id,
                document_revision: review.document_revision,
                table: review.table,
                effect: QueryActionEffect::Update {
                    matched_rows: review.matched_rows,
                    changed_rows: review.changed_rows,
                },
            },
            ReviewedAction::Update(capability),
        );
    }

    pub(in crate::workspace) fn publish_query_delete_review(
        &mut self,
        request_id: u64,
        review: RuntimeDeleteReview,
        capability: ReviewedQueryDelete,
    ) {
        self.publish_query_action_review(
            request_id,
            QueryActionReview {
                id: review.id,
                document_revision: review.document_revision,
                table: review.table,
                effect: QueryActionEffect::Delete {
                    matched_rows: review.matched_rows,
                    deleted_rows: review.deleted_rows,
                },
            },
            ReviewedAction::Delete(capability),
        );
    }

    pub(in crate::workspace) fn publish_query_insert_review(
        &mut self,
        request_id: u64,
        review: RuntimeInsertReview,
        capability: ReviewedQueryInsert,
    ) {
        self.publish_query_action_review(
            request_id,
            QueryActionReview {
                id: review.id,
                document_revision: review.document_revision,
                table: review.table,
                effect: QueryActionEffect::Insert {
                    inserted_rows: review.inserted_rows,
                },
            },
            ReviewedAction::Insert(capability),
        );
    }

    fn publish_query_action_review(
        &mut self,
        request_id: u64,
        review: QueryActionReview,
        capability: ReviewedAction,
    ) {
        if review.effect.kind() != capability.kind()
            || review.document_revision != capability.document_revision()
            || review.table != capability.table()
        {
            return;
        }
        let current =
            self.editing
                .query_action_pending
                .as_ref()
                .is_some_and(|(expected, target)| {
                    *expected == request_id
                        && action_kind(target) == Some(capability.kind())
                        && self
                            .state
                            .query_action_review_current(request_id, target.sql())
                });
        if !current
            || !matches!(self.pending, PendingOperation::QueryActionReview { request_id: expected } if expected == request_id)
        {
            return;
        }
        let id = review.id;
        if self.state.set_query_action_review(request_id, review) {
            self.editing.query_action_review = Some((id, capability));
        } else {
            self.release_stale_query_action_review();
        }
    }

    pub(in crate::workspace) fn fail_query_action_review(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        if self.editing.query_action_review.is_some()
            || !self
                .editing
                .query_action_pending
                .as_ref()
                .is_some_and(|(expected, target)| {
                    *expected == request_id
                        && self
                            .state
                            .query_action_review_current(request_id, target.sql())
                })
            || !matches!(self.pending, PendingOperation::QueryActionReview { request_id: expected } if expected == request_id)
        {
            return;
        }
        self.state.fail_query_action_review(request_id, message);
        self.editing.query_action_pending = None;
        self.editing.query_action_review = None;
        self.pending = PendingOperation::Read;
    }

    pub(in crate::workspace) fn cancel_query_action_review(&mut self, request_id: u64) {
        if self.editing.has_save()
            || !self
                .editing
                .query_action_pending
                .as_ref()
                .is_some_and(|(expected, _)| *expected == request_id)
        {
            return;
        }
        self.state.close_query_action();
        self.editing.query_action_pending = None;
        self.editing.query_action_review = None;
        if matches!(self.pending, PendingOperation::QueryActionReview { request_id: expected } if expected == request_id)
            && self.generation == request_id
        {
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn apply_query_action(&mut self, review_id: u64) {
        if self.editing.has_save() || !self.state.can_apply_query_action(review_id) {
            return;
        }
        let Some((id, capability)) = self.editing.query_action_review.take() else {
            return;
        };
        let source_matches =
            self.state.database.as_ref().is_some_and(|database| {
                database.document_revision == capability.document_revision()
            });
        if id != review_id || !source_matches {
            self.editing.query_action_review = Some((id, capability));
            return;
        }
        let path = capability.source_path().to_path_buf();
        let table = capability.table().to_owned();
        match capability.start(&self.editing.worker, journal_path(&path)) {
            Ok(save_id) => {
                self.editing.query_action_pending = None;
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::QueryAction,
                    id: save_id,
                    path,
                    table,
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                self.state.begin_query_action_save(review_id, save_id);
                self.pending = PendingOperation::Read;
                self.state.busy = false;
            }
            Err((message, capability)) => {
                self.editing.query_action_review = capability.map(|capability| (id, capability));
                self.state.fail_query_action_apply(review_id, message);
            }
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
