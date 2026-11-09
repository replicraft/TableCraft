//! Reviewed SQL action decisions bound to the original scratch acquisition.
//! No retained Access warning capture exists; dialog layout is an adaptation.
mod dialog;

use crate::workspace::objects::MAX_RETAINED_BYTES;
use crate::workspace::objects::memory::Counter;
use crate::workspace::query::parameters::{ParameterInput, retained_inputs};
use crate::workspace::{DocumentView, EditSaveOutcome, ObjectTab, State};

const MAX_REPORT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryActionKind {
    Update,
    Delete,
    Insert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryActionEffect {
    /// Qualified append count, without a matched-old-row count or UI-imposed limit.
    Insert { inserted_rows: usize },
    Update {
        matched_rows: usize,
        changed_rows: usize,
    },
    Delete {
        matched_rows: usize,
        deleted_rows: usize,
    },
}

impl QueryActionEffect {
    pub fn kind(self) -> QueryActionKind {
        match self {
            Self::Update { .. } => QueryActionKind::Update,
            Self::Delete { .. } => QueryActionKind::Delete,
            Self::Insert { .. } => QueryActionKind::Insert,
        }
    }

    fn valid(self) -> bool {
        match self {
            Self::Insert { .. } => true,
            Self::Update {
                matched_rows,
                changed_rows,
            } => changed_rows <= matched_rows,
            Self::Delete {
                matched_rows,
                deleted_rows,
            } => deleted_rows == matched_rows,
        }
    }

    fn matched_rows(self) -> Option<usize> {
        match self {
            Self::Update { matched_rows, .. } | Self::Delete { matched_rows, .. } => {
                Some(matched_rows)
            }
            Self::Insert { .. } => None,
        }
    }
}

pub struct QueryActionReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub effect: QueryActionEffect,
}

pub(in crate::workspace) struct Origin {
    pub(in crate::workspace) sql: String,
    pub(in crate::workspace) sql_revision: u64,
    pub(in crate::workspace) object: ObjectTab,
    pub(in crate::workspace) publication: u64,
    pub(in crate::workspace) parameters: Vec<ParameterInput>,
}

impl Origin {
    fn current(&self, state: &State) -> bool {
        state.focus_publication == self.publication
            && state.active_object().as_ref() == Some(&self.object)
            && state.view == DocumentView::Query
            && !state.saved_query.active
            && !state.backstage_visible()
            && !state.query_design_visible()
            && !state.query_design_results_visible()
            && state.query_sql_witness_current(self.sql_revision, &self.sql)
    }
}

pub(in crate::workspace) struct Session {
    origin: Origin,
    kind: QueryActionKind,
    phase: Phase,
}

enum Phase {
    Reviewing {
        request_id: u64,
        error: Option<Report>,
    },
    Review {
        request_id: u64,
        review: QueryActionReview,
        error: Option<Report>,
        focus_no: bool,
    },
    Saving {
        save_id: u64,
        cancel_requested: bool,
        resolving: bool,
    },
    Finished {
        save_id: u64,
        outcome: EditSaveOutcome,
        report_refused: bool,
    },
}

enum Report {
    Message(String),
    Limit,
}

impl Report {
    fn retained_bytes(&self) -> usize {
        match self {
            Self::Message(message) => message.capacity(),
            Self::Limit => 0,
        }
    }
}

fn outcome_message(outcome: &EditSaveOutcome) -> &String {
    match outcome {
        EditSaveOutcome::Retained { message, .. }
        | EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Failed(message) => message,
    }
}

impl Session {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(self.origin.sql.capacity());
        count.add(retained_inputs(&self.origin.parameters)?);
        match &self.phase {
            Phase::Reviewing { error, .. } => {
                if let Some(error) = error {
                    count.add(error.retained_bytes());
                }
            }
            Phase::Review { review, error, .. } => {
                count.string(&review.table);
                if let Some(error) = error {
                    count.add(error.retained_bytes());
                }
            }
            Phase::Saving { .. } => {}
            Phase::Finished { outcome, .. } => count.string(outcome_message(outcome)),
        }
        count.finish()
    }

    fn pending(&self) -> bool {
        matches!(self.phase, Phase::Saving { .. } | Phase::Finished { .. })
    }
}

impl State {
    /// Transfer the original inspection witness, rather than recapturing a draft.
    pub fn begin_query_action_review(
        &mut self,
        inspection_id: u64,
        sql: &str,
        request_id: u64,
        parameters: Vec<ParameterInput>,
        kind: QueryActionKind,
    ) -> bool {
        if self.number_column_source_stale() || request_id == 0 || self.query_action.is_some() {
            return false;
        }
        let Some(origin) = self.take_query_action_origin(inspection_id, sql, &parameters) else {
            return false;
        };
        self.query_action = Some(Session {
            origin,
            kind,
            phase: Phase::Reviewing {
                request_id,
                error: None,
            },
        });
        true
    }

    pub fn query_action_review_current(&self, request_id: u64, sql: &str) -> bool {
        self.query_action.as_ref().is_some_and(|session| {
            session.origin.sql == sql && session.origin.current(self)
                && matches!(&session.phase, Phase::Reviewing { request_id: expected, .. } | Phase::Review { request_id: expected, .. } if *expected == request_id)
        })
    }

    pub fn set_query_action_review(&mut self, request_id: u64, review: QueryActionReview) -> bool {
        let accepts = self.query_action.as_ref().is_some_and(|session| {
            session.origin.current(self)
                && matches!(session.phase, Phase::Reviewing { request_id: expected, .. } if expected == request_id)
        });
        if !accepts {
            return false;
        }
        let valid = self.query_action.as_ref().is_some_and(|session| {
            review.id != 0
                && review.document_revision == session.origin.object.target.document_revision
                && !review.table.is_empty()
                && review.effect.kind() == session.kind
                && review.effect.valid()
        });
        if !valid {
            self.fail_query_action_review(
                request_id,
                self.localizer.text("query-action-invalid-review"),
            );
            return false;
        }
        if !self.admit_object_bytes(Some(review.table.capacity())) {
            self.fail_query_action_review(request_id, self.localizer.text("objects-memory-limit"));
            return false;
        }
        let session = self
            .query_action
            .as_mut()
            .expect("checked action review owner");
        session.phase = Phase::Review {
            request_id,
            review,
            error: None,
            focus_no: true,
        };
        true
    }

    pub fn fail_query_action_review(&mut self, request_id: u64, message: String) {
        let replaced = match &self.query_action {
            Some(Session {
                phase:
                    Phase::Reviewing {
                        request_id: expected,
                        error,
                    },
                ..
            }) if *expected == request_id => error.as_ref().map_or(0, Report::retained_bytes),
            _ => return,
        };
        let report = self.normalize_action_report(message, replaced);
        if let Some(Session {
            phase: Phase::Reviewing { error, .. },
            ..
        }) = &mut self.query_action
        {
            *error = Some(report);
        }
    }

    pub fn can_apply_query_action(&self, review_id: u64) -> bool {
        if self.number_column_source_stale() || self.has_modified_saved_query_sql() {
            return false;
        }

        !self.busy && self.query_action.as_ref().is_some_and(|session| {
            session.origin.current(self)
                && matches!(&session.phase, Phase::Review { review, .. } if review.id == review_id)
        })
    }

    pub fn fail_query_action_apply(&mut self, review_id: u64, message: String) {
        let replaced = match &self.query_action {
            Some(Session {
                phase: Phase::Review { review, error, .. },
                ..
            }) if review.id == review_id => error.as_ref().map_or(0, Report::retained_bytes),
            _ => return,
        };
        let report = self.normalize_action_report(message, replaced);
        if let Some(Session {
            phase: Phase::Review { error, .. },
            ..
        }) = &mut self.query_action
        {
            *error = Some(report);
        }
    }

    pub fn begin_query_action_save(&mut self, review_id: u64, save_id: u64) -> bool {
        if save_id == 0 || !self.can_apply_query_action(review_id) {
            return false;
        }
        self.query_action
            .as_mut()
            .expect("checked action review owner")
            .phase = Phase::Saving {
            save_id,
            cancel_requested: false,
            resolving: false,
        };
        true
    }

    pub fn set_query_action_save_outcome(
        &mut self,
        save_id: u64,
        outcome: EditSaveOutcome,
    ) -> bool {
        let replaced = match &self.query_action {
            Some(Session {
                phase: Phase::Saving {
                    save_id: expected, ..
                },
                ..
            }) if *expected == save_id => 0,
            Some(Session {
                phase:
                    Phase::Finished {
                        save_id: expected,
                        outcome,
                        ..
                    },
                ..
            }) if *expected == save_id => outcome_message(outcome).capacity(),
            _ => return false,
        };
        let mut report_refused = false;
        let mut normalize = |message| match self.normalize_action_report(message, replaced) {
            Report::Message(message) => message,
            Report::Limit => {
                report_refused = true;
                String::new()
            }
        };
        let outcome = match outcome {
            EditSaveOutcome::Retained {
                message,
                can_recover,
                can_release,
            } => EditSaveOutcome::Retained {
                message: normalize(message),
                can_recover,
                can_release,
            },
            EditSaveOutcome::Saved(message) => EditSaveOutcome::Saved(normalize(message)),
            EditSaveOutcome::Cancelled(message) => EditSaveOutcome::Cancelled(normalize(message)),
            EditSaveOutcome::Failed(message) => EditSaveOutcome::Failed(normalize(message)),
        };
        self.query_action
            .as_mut()
            .expect("checked action save owner")
            .phase = Phase::Finished {
            save_id,
            outcome,
            report_refused,
        };
        true
    }

    fn normalize_action_report(&self, message: String, replaced_bytes: usize) -> Report {
        let available = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_sub(replaced_bytes))
            .and_then(|bytes| MAX_RETAINED_BYTES.checked_sub(bytes));
        if message.capacity() <= MAX_REPORT_BYTES
            && available.is_some_and(|bytes| message.capacity() <= bytes)
        {
            Report::Message(message)
        } else {
            Report::Limit
        }
    }

    pub fn begin_query_action_resolution(&mut self, save_id: u64, operation_id: u64) -> bool {
        let Some(session) = &mut self.query_action else {
            return false;
        };
        if operation_id == 0
            || !matches!(session.phase, Phase::Finished { save_id: expected, outcome: EditSaveOutcome::Retained { .. }, .. } if expected == save_id)
        {
            return false;
        }
        session.phase = Phase::Saving {
            save_id: operation_id,
            cancel_requested: false,
            resolving: true,
        };
        true
    }

    pub fn query_action_save_id(&self) -> Option<u64> {
        match &self.query_action.as_ref()?.phase {
            Phase::Saving { save_id, .. } | Phase::Finished { save_id, .. } => Some(*save_id),
            Phase::Reviewing { .. } | Phase::Review { .. } => None,
        }
    }

    pub fn request_query_action_save_cancellation(&mut self, save_id: u64) -> bool {
        if let Some(Session {
            phase:
                Phase::Saving {
                    save_id: expected,
                    cancel_requested,
                    ..
                },
            ..
        }) = &mut self.query_action
            && *expected == save_id
        {
            *cancel_requested = true;
            return true;
        }
        false
    }

    pub fn can_resolve_query_action_save(&self, save_id: u64, recover: bool) -> bool {
        matches!(&self.query_action,
            Some(Session { phase: Phase::Finished { save_id: expected,
                outcome: EditSaveOutcome::Retained { can_recover, can_release, .. }, .. }, .. })
                if *expected == save_id && if recover { *can_recover } else { *can_release })
    }

    pub fn can_acknowledge_query_action_save(&self, save_id: u64) -> bool {
        self.query_action.as_ref().is_some_and(|session| {
            matches!(&session.phase, Phase::Finished { save_id: expected, outcome, .. }
                if *expected == save_id && !matches!(outcome, EditSaveOutcome::Retained { .. }))
        })
    }

    pub fn query_action_dialog_open(&self) -> bool {
        self.query_action.is_some()
    }

    pub fn query_action_save_pending(&self) -> bool {
        self.query_action.as_ref().is_some_and(Session::pending)
    }

    /// Clear a terminal display only after the host acknowledges its save owner.
    pub fn acknowledge_query_action_save(&mut self, save_id: u64) -> bool {
        if !self.can_acknowledge_query_action_save(save_id) {
            return false;
        }
        self.query_action = None;
        true
    }

    pub fn close_query_action(&mut self) -> bool {
        if self.query_action_save_pending() {
            return false;
        }
        self.query_action = None;
        true
    }

    pub(in crate::workspace) fn validate_query_action(&mut self) {
        if self.query_action.as_ref().is_some_and(|session| {
            matches!(
                session.phase,
                Phase::Reviewing { .. } | Phase::Review { .. }
            ) && !session.origin.current(self)
        }) {
            self.query_action = None;
        }
    }
}

#[cfg(test)]
mod tests;
