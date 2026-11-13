//! Terminal outcomes retain their own exact request and actor identity across view changes.
use super::super::{Action, EditSaveOutcome, State};
use super::QueryCreationRequest;
use crate::workspace::saved_query_save::{outcome_bytes, without_report};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryCreationSaveOperation {
    PreparedReview,
    NativeSave { save_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryCreationSaveIdentity {
    pub request_id: u64,
    pub review_id: u64,
    pub operation: QueryCreationSaveOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryCreationSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToDraft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Preparing,
    Saving { cancelled: bool },
    Finished,
    Reopening { cancelled: bool },
    ReopenFailed,
}

pub(super) struct SaveStatus {
    pub(super) identity: QueryCreationSaveIdentity,
    pub(super) request: Arc<QueryCreationRequest>,
    pub(super) phase: Phase,
    pub(super) outcome: Option<EditSaveOutcome>,
    pub(super) reopen_error: Option<String>,
}

impl State {
    pub fn prepare_query_creation_save(
        &mut self,
        request_id: u64,
        request: &QueryCreationRequest,
        review_id: u64,
    ) -> Option<QueryCreationSaveIdentity> {
        if self.query_creation.save.is_some()
            || !self.query_creation_request_current(request)
            || !self.matches_query_creation_review(request_id, request, review_id)
        {
            return None;
        }
        let pending = self.query_creation.pending.as_ref()?;
        // NO_CLONE: The terminal save status must survive release of the asynchronous
        // review association. Both retain the same immutable bounded request payload.
        let request = Arc::clone(&pending.request);
        let identity = QueryCreationSaveIdentity {
            request_id,
            review_id,
            operation: QueryCreationSaveOperation::PreparedReview,
        };
        self.query_creation.save = Some(SaveStatus {
            identity,
            request,
            phase: Phase::Preparing,
            outcome: None,
            reopen_error: None,
        });
        Some(identity)
    }

    pub fn bind_query_creation_save(
        &mut self,
        prepared: QueryCreationSaveIdentity,
        save_id: u64,
    ) -> Option<QueryCreationSaveIdentity> {
        let status = self.query_creation.save.as_mut()?;
        if status.identity != prepared || status.phase != Phase::Preparing || save_id == 0 {
            return None;
        }
        status.identity.operation = QueryCreationSaveOperation::NativeSave { save_id };
        status.phase = Phase::Saving { cancelled: false };
        Some(status.identity)
    }

    pub fn abort_query_creation_save(&mut self, identity: QueryCreationSaveIdentity) -> bool {
        let matches =
            self.query_creation.save.as_ref().is_some_and(|status| {
                status.identity == identity && status.phase == Phase::Preparing
            });
        if matches {
            self.query_creation.save = None;
        }
        matches
    }

    pub fn query_creation_save_open(&self) -> bool {
        self.query_creation.save.is_some()
    }

    pub fn query_creation_save_status(
        &self,
    ) -> Option<(QueryCreationSaveIdentity, Option<&EditSaveOutcome>)> {
        let status = self.query_creation.save.as_ref()?;
        Some((status.identity, status.outcome.as_ref()))
    }

    pub fn query_creation_save_request(
        &self,
        identity: QueryCreationSaveIdentity,
    ) -> Option<&QueryCreationRequest> {
        self.query_creation
            .save
            .as_ref()
            .filter(|status| status.identity == identity)
            .map(|status| status.request.as_ref())
    }

    pub fn matches_query_creation_save(&self, identity: QueryCreationSaveIdentity) -> bool {
        self.query_creation
            .save
            .as_ref()
            .is_some_and(|status| status.identity == identity)
    }

    pub fn set_query_creation_actor_lost(
        &mut self,
        identity: QueryCreationSaveIdentity,
        message: String,
    ) -> bool {
        if !self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity
                && matches!(status.phase, Phase::Saving { .. } | Phase::Finished)
        }) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let status = self.query_creation.save.as_mut().expect("checked save");
        status.outcome = Some(EditSaveOutcome::Failed(if admitted {
            message
        } else {
            String::new()
        }));
        status.phase = Phase::Finished;
        self.mark_native_save_uncertain();
        true
    }

    pub fn set_query_creation_save_outcome(
        &mut self,
        identity: QueryCreationSaveIdentity,
        outcome: EditSaveOutcome,
    ) -> bool {
        if !self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity
                && matches!(
                    identity.operation,
                    QueryCreationSaveOperation::NativeSave { .. }
                )
                && matches!(status.phase, Phase::Saving { .. } | Phase::Finished)
        }) {
            return false;
        }
        let outcome = if self.admit_object_bytes(Some(outcome_bytes(&outcome))) {
            outcome
        } else {
            without_report(outcome)
        };
        let status = self.query_creation.save.as_mut().expect("checked save");
        status.outcome = Some(outcome);
        status.phase = Phase::Finished;
        status.reopen_error = None;
        true
    }

    pub fn can_resolve_query_creation_save(
        &self,
        identity: QueryCreationSaveIdentity,
        recover: bool,
    ) -> bool {
        self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::Finished
                && matches!(&status.outcome, Some(EditSaveOutcome::Retained { can_recover, can_release, .. })
                    if if recover { *can_recover } else { *can_release })
        })
    }

    pub fn begin_query_creation_save_resolution(
        &mut self,
        identity: QueryCreationSaveIdentity,
        next_id: u64,
    ) -> Option<QueryCreationSaveIdentity> {
        if next_id == 0
            || !(self.can_resolve_query_creation_save(identity, true)
                || self.can_resolve_query_creation_save(identity, false))
            || matches!(identity.operation, QueryCreationSaveOperation::NativeSave { save_id } if save_id == next_id)
        {
            return None;
        }
        let status = self.query_creation.save.as_mut()?;
        status.identity.operation = QueryCreationSaveOperation::NativeSave { save_id: next_id };
        status.phase = Phase::Saving { cancelled: false };
        Some(status.identity)
    }

    pub fn can_acknowledge_query_creation_save(&self, identity: QueryCreationSaveIdentity) -> bool {
        self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity
                && status.phase == Phase::Finished
                && matches!(
                    status.outcome,
                    Some(
                        EditSaveOutcome::Saved(_)
                            | EditSaveOutcome::Cancelled(_)
                            | EditSaveOutcome::Failed(_)
                    )
                )
        })
    }

    pub fn begin_query_creation_reopen(&mut self, identity: QueryCreationSaveIdentity) -> bool {
        let retry = self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::ReopenFailed
        });
        if !retry && !self.can_acknowledge_query_creation_save(identity) {
            return false;
        }
        let status = self.query_creation.save.as_mut().expect("checked save");
        status.phase = Phase::Reopening { cancelled: false };
        status.reopen_error = None;
        true
    }

    pub fn set_query_creation_reopen_failed(
        &mut self,
        identity: QueryCreationSaveIdentity,
        message: String,
    ) -> bool {
        if !self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity && matches!(status.phase, Phase::Reopening { .. })
        }) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let status = self.query_creation.save.as_mut().expect("checked save");
        status.reopen_error = Some(if admitted { message } else { String::new() });
        status.phase = Phase::ReopenFailed;
        true
    }

    pub fn query_creation_reopen_error(&self, identity: QueryCreationSaveIdentity) -> Option<&str> {
        self.query_creation
            .save
            .as_ref()
            .filter(|status| status.identity == identity)?
            .reopen_error
            .as_deref()
    }

    pub fn can_return_to_query_creation_draft(&self, identity: QueryCreationSaveIdentity) -> bool {
        self.query_creation.save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::ReopenFailed
        })
    }

    pub fn clear_query_creation_save(&mut self, identity: QueryCreationSaveIdentity) -> bool {
        let clearable = self.can_acknowledge_query_creation_save(identity)
            || self.query_creation.save.as_ref().is_some_and(|status| {
                status.identity == identity
                    && matches!(status.phase, Phase::Reopening { .. } | Phase::ReopenFailed)
            });
        if clearable {
            self.cancel_query_design_close_creation(identity.request_id);
            self.query_creation.save = None;
            self.query_creation.pending = None;
        }
        clearable
    }

    pub fn request_query_creation_save_command(
        &mut self,
        identity: QueryCreationSaveIdentity,
        command: QueryCreationSaveCommand,
    ) -> Option<Action> {
        let status = self
            .query_creation
            .save
            .as_ref()
            .filter(|status| status.identity == identity)?;
        let permitted = match command {
            QueryCreationSaveCommand::Cancel => matches!(
                status.phase,
                Phase::Saving { cancelled: false } | Phase::Reopening { cancelled: false }
            ),
            QueryCreationSaveCommand::Recover => {
                self.can_resolve_query_creation_save(identity, true)
            }
            QueryCreationSaveCommand::Release => {
                self.can_resolve_query_creation_save(identity, false)
            }
            QueryCreationSaveCommand::Acknowledge => {
                self.can_acknowledge_query_creation_save(identity)
            }
            QueryCreationSaveCommand::RetryReopen | QueryCreationSaveCommand::ReturnToDraft => {
                self.can_return_to_query_creation_draft(identity)
            }
        };
        if !permitted {
            return None;
        }
        if command == QueryCreationSaveCommand::Cancel {
            let status = self.query_creation.save.as_mut()?;
            match &mut status.phase {
                Phase::Saving { cancelled } | Phase::Reopening { cancelled } => *cancelled = true,
                _ => return None,
            }
        }
        Some(Action::QueryCreationSave { identity, command })
    }
}

impl State {
    pub(in crate::workspace) fn query_creation_reopen_request(
        &self,
        identity: QueryCreationSaveIdentity,
    ) -> Option<Arc<QueryCreationRequest>> {
        let status = self.query_creation.save.as_ref()?;
        if status.identity != identity
            || !matches!(status.phase, Phase::Reopening { .. })
            || !self.query_creation_owner_current(&status.request)
        {
            return None;
        }
        // NO_CLONE: Publication prepares a separate mutable workspace graph while
        // the status must retain the same immutable request until atomic success.
        Some(Arc::clone(&status.request))
    }
}
