use super::{FieldDropRequest, FieldDropSaveIdentity, model::Phase};
use crate::workspace::{EditSaveOutcome, State, schema_save::outcome_bytes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldDropSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToDraft,
}

impl State {
    pub fn matches_field_drop_save(
        &self,
        request: &FieldDropRequest,
        identity: FieldDropSaveIdentity,
    ) -> bool {
        self.field_drop.matches(request, Phase::Owned { identity })
            && self
                .field_drop
                .status
                .as_ref()
                .is_some_and(|status| status.identity == identity)
    }

    pub fn field_drop_save_status(
        &self,
    ) -> Option<(FieldDropSaveIdentity, Option<&EditSaveOutcome>)> {
        let status = self.field_drop.status.as_ref()?;
        Some((status.identity, status.outcome.as_ref()))
    }

    pub fn field_drop_status_open(&self) -> bool {
        self.field_drop.status.is_some()
    }

    pub fn field_drop_modal_open(&self) -> bool {
        self.field_drop.status.is_some()
            || self.field_drop_confirmation_open()
            || self.field_drop.draft.as_ref().is_some_and(|draft| {
                matches!(
                    draft.phase,
                    Phase::Reviewing { .. } | Phase::Reviewed { .. }
                )
            })
    }

    pub fn set_field_drop_save_outcome(
        &mut self,
        identity: FieldDropSaveIdentity,
        outcome: EditSaveOutcome,
    ) -> bool {
        if !self
            .field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.accepts_outcome(identity))
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        self.field_drop
            .status
            .as_mut()
            .unwrap()
            .record_outcome(outcome, admitted);
        true
    }

    pub fn can_resolve_field_drop_save(
        &self,
        identity: FieldDropSaveIdentity,
        recover: bool,
    ) -> bool {
        self.field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.can_resolve(identity, recover))
    }

    pub fn can_acknowledge_field_drop_save(&self, identity: FieldDropSaveIdentity) -> bool {
        self.field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.can_acknowledge(identity))
    }

    /// Host prerequisite: real actor acknowledged/released with known AFTER.
    /// Keep the pending mark and outcome until the full FieldDroppedReopen witness
    /// and operation-specific publication succeed. A report alone is insufficient.
    pub fn begin_field_drop_reopen(&mut self, identity: FieldDropSaveIdentity) -> bool {
        let Some(status) = self.field_drop.status.as_mut() else {
            return false;
        };
        if !status.can_acknowledge(identity) && !status.can_retry_reopen(identity) {
            return false;
        }
        status.begin_reopen(identity)
    }

    pub fn fail_field_drop_reopen(
        &mut self,
        identity: FieldDropSaveIdentity,
        message: String,
    ) -> bool {
        if !self
            .field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.reopening(identity))
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_drop
            .status
            .as_mut()
            .unwrap()
            .record_reopen_failure(message, admitted);
        true
    }

    pub(in crate::workspace) fn field_drop_reopening(
        &self,
        identity: FieldDropSaveIdentity,
    ) -> bool {
        self.field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.reopening(identity))
    }

    pub fn can_retry_field_drop_reopen(&self, identity: FieldDropSaveIdentity) -> bool {
        self.field_drop
            .status
            .as_ref()
            .is_some_and(|status| status.can_retry_reopen(identity))
    }

    pub fn cancel_field_drop_operation(&mut self, identity: FieldDropSaveIdentity) -> bool {
        self.field_drop
            .status
            .as_mut()
            .is_some_and(|status| status.cancel(identity))
    }

    /// Host prerequisite: exact actor ACK has verified original/no-write.
    /// Keep the confirmed deletion dirty; another Save requires a fresh review.
    pub fn release_field_drop_save(
        &mut self,
        request: &FieldDropRequest,
        identity: FieldDropSaveIdentity,
    ) -> bool {
        if !self.matches_field_drop_save(request, identity)
            || !self.can_acknowledge_field_drop_save(identity)
        {
            return false;
        }
        self.field_drop.draft.as_mut().unwrap().phase = Phase::PendingDeletion;
        self.field_drop.status = None;
        true
    }

    /// Presentation-only completion after successful operation-specific publication.
    /// Never call on a receipt, ACK, failed read or plain Database observation.
    pub(in crate::workspace) fn finish_field_drop_publication(&mut self) {
        self.field_drop = super::model::Model::default();
    }
}
