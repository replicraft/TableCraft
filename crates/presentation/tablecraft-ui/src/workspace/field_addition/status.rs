//! Resource-free status facts keyed to the actual field-addition save actor.
mod preparation;
use super::{FieldAdditionRequest, Phase};
use crate::workspace::{EditSaveOutcome, State};
pub use preparation::PreparedFieldAdditionSave;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldAdditionSaveIdentity {
    pub request_id: u64,
    pub review_id: u64,
    pub save_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToDraft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StatusPhase {
    Saving { cancelled: bool, resolving: bool },
    Finished,
    Reopening { cancelled: bool },
    ReopenFailed,
}

pub(in crate::workspace) struct Status {
    pub(super) identity: FieldAdditionSaveIdentity,
    pub(super) phase: StatusPhase,
    pub(super) outcome: Option<EditSaveOutcome>,
    pub(super) error: Option<String>,
    pub(super) omitted: bool,
}

impl Status {
    pub(in crate::workspace) fn heap_bytes(&self) -> Option<usize> {
        self.outcome
            .as_ref()
            .map_or(0, outcome_bytes)
            .checked_add(self.error.as_ref().map_or(0, String::capacity))
    }
}

impl State {
    pub fn matches_field_addition_save(
        &self,
        request: &FieldAdditionRequest,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        self.field_addition.matches(
            request,
            Phase::Saving {
                request_id: identity.request_id,
                review_id: identity.review_id,
                save_id: identity.save_id,
            },
        ) && self
            .field_addition_status
            .as_ref()
            .is_some_and(|status| status.identity == identity)
    }

    /// Nonallocating, synchronous admission after a prepared review starts the
    /// actual actor. The naming model remains the owner until witnessed reopen.
    pub fn begin_field_addition_save_status(
        &mut self,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        if self.field_addition_status.is_some()
            || !self.field_addition.draft().is_some_and(|(_, _, _, phase)| {
                phase
                    == Phase::Saving {
                        request_id: identity.request_id,
                        review_id: identity.review_id,
                        save_id: identity.save_id,
                    }
            })
        {
            return false;
        }
        self.field_addition_status = Some(Status {
            identity,
            phase: StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
            outcome: None,
            error: None,
            omitted: false,
        });
        true
    }

    pub fn field_addition_save_status(
        &self,
    ) -> Option<(FieldAdditionSaveIdentity, Option<&EditSaveOutcome>)> {
        let status = self.field_addition_status.as_ref()?;
        Some((status.identity, status.outcome.as_ref()))
    }

    pub fn set_field_addition_save_outcome(
        &mut self,
        identity: FieldAdditionSaveIdentity,
        mut outcome: EditSaveOutcome,
    ) -> bool {
        if !self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity && matches!(status.phase, StatusPhase::Saving { .. })
        }) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        if !admitted {
            match &mut outcome {
                EditSaveOutcome::Saved(message)
                | EditSaveOutcome::Failed(message)
                | EditSaveOutcome::Cancelled(message)
                | EditSaveOutcome::Retained { message, .. } => *message = String::new(),
            }
        }
        let status = self.field_addition_status.as_mut().unwrap();
        status.omitted = !admitted;
        status.outcome = Some(outcome);
        status.phase = StatusPhase::Finished;
        true
    }

    pub fn can_resolve_field_addition_save(
        &self,
        identity: FieldAdditionSaveIdentity,
        recover: bool,
    ) -> bool {
        self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == StatusPhase::Finished
                && matches!(&status.outcome, Some(EditSaveOutcome::Retained { can_recover, can_release, .. })
                    if if recover { *can_recover } else { *can_release })
        })
    }

    pub fn begin_field_addition_resolution(
        &mut self,
        request: &FieldAdditionRequest,
        identity: FieldAdditionSaveIdentity,
        next_id: u64,
        recover: bool,
    ) -> Option<FieldAdditionSaveIdentity> {
        if !self.matches_field_addition_save(request, identity)
            || !self.can_resolve_field_addition_save(identity, recover)
        {
            return None;
        }
        let next = FieldAdditionSaveIdentity {
            save_id: next_id,
            ..identity
        };
        self.field_addition.draft.as_mut()?.phase = Phase::Saving {
            request_id: next.request_id,
            review_id: next.review_id,
            save_id: next.save_id,
        };
        let status = self.field_addition_status.as_mut()?;
        status.identity = next;
        status.phase = StatusPhase::Saving {
            cancelled: false,
            resolving: true,
        };
        status.outcome = None;
        Some(next)
    }

    pub fn can_acknowledge_field_addition_save(&self, identity: FieldAdditionSaveIdentity) -> bool {
        self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity
                && status.phase == StatusPhase::Finished
                && status
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| !matches!(outcome, EditSaveOutcome::Retained { .. }))
        })
    }

    /// A native ACK must precede this transition; it alone does not resolve the
    /// field draft. Keep the known terminal outcome while the witness is read.
    pub fn begin_field_addition_reopen(&mut self, identity: FieldAdditionSaveIdentity) -> bool {
        let Some(status) = self
            .field_addition_status
            .as_mut()
            .filter(|status| status.identity == identity)
        else {
            return false;
        };
        if status.phase != StatusPhase::Finished && status.phase != StatusPhase::ReopenFailed {
            return false;
        }
        status.phase = StatusPhase::Reopening { cancelled: false };
        status.error = None;
        true
    }

    pub fn fail_field_addition_reopen(
        &mut self,
        identity: FieldAdditionSaveIdentity,
        message: String,
    ) -> bool {
        if !self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity && matches!(status.phase, StatusPhase::Reopening { .. })
        }) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let status = self.field_addition_status.as_mut().unwrap();
        status.phase = StatusPhase::ReopenFailed;
        status.error = Some(if admitted { message } else { String::new() });
        status.omitted |= !admitted;
        true
    }

    pub fn can_retry_field_addition_reopen(&self, identity: FieldAdditionSaveIdentity) -> bool {
        self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == StatusPhase::ReopenFailed
        })
    }

    pub fn cancel_field_addition_operation(&mut self, identity: FieldAdditionSaveIdentity) -> bool {
        let Some(status) = self
            .field_addition_status
            .as_mut()
            .filter(|status| status.identity == identity)
        else {
            return false;
        };
        match &mut status.phase {
            StatusPhase::Saving { cancelled, .. } | StatusPhase::Reopening { cancelled }
                if !*cancelled =>
            {
                *cancelled = true;
                true
            }
            _ => false,
        }
    }

    /// Original/no-write ACK releases only the actor. Keep the unsaved name;
    /// another explicit Save must acquire a completely fresh native review.
    pub fn release_field_addition_save(
        &mut self,
        request: &FieldAdditionRequest,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        if !self.matches_field_addition_save(request, identity)
            || !self.can_acknowledge_field_addition_save(identity)
        {
            return false;
        }
        self.field_addition.draft.as_mut().unwrap().phase = Phase::Naming;
        self.field_addition_status = None;
        true
    }

    /// Host precondition: either native original/no-write ACK, or successful
    /// full-witness schema publication. No receipt or presentation fact suffices.
    pub fn clear_field_addition_save(
        &mut self,
        request: &FieldAdditionRequest,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        if !self.matches_field_addition_save(request, identity) {
            return false;
        }
        self.field_addition = super::Model::default();
        self.field_addition_status = None;
        true
    }
}

fn outcome_bytes(outcome: &EditSaveOutcome) -> usize {
    match outcome {
        EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Failed(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Retained { message, .. } => message.capacity(),
    }
}
