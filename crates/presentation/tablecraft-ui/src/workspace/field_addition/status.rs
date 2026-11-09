//! Resource-free status facts keyed to the actual field-addition save actor.
mod preparation;
use super::{FieldAdditionRequest, Phase};
use crate::workspace::{
    EditSaveOutcome, State,
    schema_save::{Status, StatusPhase, outcome_bytes},
};
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
        outcome: EditSaveOutcome,
    ) -> bool {
        if !self
            .field_addition_status
            .as_ref()
            .is_some_and(|status| status.accepts_outcome(identity))
        {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        self.field_addition_status
            .as_mut()
            .unwrap()
            .record_outcome(outcome, admitted);

        true
    }

    /// Replace only this actual actor's presentation after observed teardown.
    /// Historical retained facts no longer authorize recover/release.
    pub fn set_field_addition_actor_lost(
        &mut self,
        identity: FieldAdditionSaveIdentity,
        message: String,
    ) -> bool {
        if !self.field_addition_status.as_ref().is_some_and(|status| {
            status.identity == identity
                && matches!(
                    status.phase,
                    crate::workspace::schema_save::StatusPhase::Saving { .. }
                        | crate::workspace::schema_save::StatusPhase::Finished
                )
        }) {
            return false;
        }

        let outcome = EditSaveOutcome::Failed(message);
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        self.field_addition_status
            .as_mut()
            .unwrap()
            .record_outcome(outcome, admitted);
        self.mark_native_save_uncertain();
        true
    }

    pub fn can_resolve_field_addition_save(
        &self,
        identity: FieldAdditionSaveIdentity,
        recover: bool,
    ) -> bool {
        self.field_addition_status
            .as_ref()
            .is_some_and(|status| status.can_resolve(identity, recover))
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
        self.field_addition_status
            .as_ref()
            .is_some_and(|status| status.can_acknowledge(identity))
    }

    /// A native ACK must precede this transition; it alone does not resolve the
    /// field draft. Keep the known terminal outcome while the witness is read.
    pub fn begin_field_addition_reopen(&mut self, identity: FieldAdditionSaveIdentity) -> bool {
        self.field_addition_status
            .as_mut()
            .is_some_and(|status| status.begin_reopen(identity))
    }

    pub fn fail_field_addition_reopen(
        &mut self,
        identity: FieldAdditionSaveIdentity,
        message: String,
    ) -> bool {
        if !self
            .field_addition_status
            .as_ref()
            .is_some_and(|status| status.reopening(identity))
        {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_addition_status
            .as_mut()
            .unwrap()
            .record_reopen_failure(message, admitted);

        true
    }

    pub fn can_retry_field_addition_reopen(&self, identity: FieldAdditionSaveIdentity) -> bool {
        self.field_addition_status
            .as_ref()
            .is_some_and(|status| status.can_retry_reopen(identity))
    }

    pub fn cancel_field_addition_operation(&mut self, identity: FieldAdditionSaveIdentity) -> bool {
        self.field_addition_status
            .as_mut()
            .is_some_and(|status| status.cancel(identity))
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

    /// Host precondition: exact uncertain actor teardown and ACK. The proposed
    /// field remains local; no original/replacement result is inferred.
    pub fn acknowledge_uncertain_field_addition_save(
        &mut self,
        request: &FieldAdditionRequest,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        if !self.matches_field_addition_save(request, identity)
            || !self.can_acknowledge_field_addition_save(identity)
        {
            return false;
        }

        self.mark_native_save_uncertain();
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
        self.finish_field_addition_presentation();
        self.field_addition_status = None;
        true
    }
}
