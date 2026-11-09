//! Exact rename ownership around the shared source-neutral save-status kernel.
use super::{FieldRenameRequest, FieldRenameSaveIdentity, model::Phase};
use crate::workspace::{EditSaveOutcome, State, schema_save::outcome_bytes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRenameSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToDraft,
}

impl State {
    pub fn matches_field_rename_save(
        &self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
    ) -> bool {
        self.field_rename
            .matches(request, Phase::Owned { identity })
            && self
                .field_rename
                .status
                .as_ref()
                .is_some_and(|status| status.identity == identity)
    }

    pub fn field_rename_save_status(
        &self,
    ) -> Option<(FieldRenameSaveIdentity, Option<&EditSaveOutcome>)> {
        let status = self.field_rename.status.as_ref()?;
        Some((status.identity, status.outcome.as_ref()))
    }

    pub fn field_rename_status_open(&self) -> bool {
        self.field_rename.status.is_some()
    }

    pub fn field_rename_modal_open(&self) -> bool {
        self.field_rename.status.is_some()
            || self.field_rename.draft.as_ref().is_some_and(|draft| {
                matches!(
                    draft.phase,
                    Phase::Reviewing { .. } | Phase::Reviewed { .. }
                )
            })
    }

    pub fn set_field_rename_save_outcome(
        &mut self,
        identity: FieldRenameSaveIdentity,
        outcome: EditSaveOutcome,
    ) -> bool {
        if !matches!(identity, FieldRenameSaveIdentity::NativeSave { .. })
            || !self
                .field_rename
                .status
                .as_ref()
                .is_some_and(|status| status.accepts_outcome(identity))
        {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        self.field_rename
            .status
            .as_mut()
            .unwrap()
            .record_outcome(outcome, admitted);

        true
    }

    pub fn can_resolve_field_rename_save(
        &self,
        identity: FieldRenameSaveIdentity,
        recover: bool,
    ) -> bool {
        matches!(identity, FieldRenameSaveIdentity::NativeSave { .. })
            && self
                .field_rename
                .status
                .as_ref()
                .is_some_and(|status| status.can_resolve(identity, recover))
    }

    pub fn can_acknowledge_field_rename_save(&self, identity: FieldRenameSaveIdentity) -> bool {
        matches!(identity, FieldRenameSaveIdentity::NativeSave { .. })
            && self
                .field_rename
                .status
                .as_ref()
                .is_some_and(|status| status.can_acknowledge(identity))
    }

    /// Native ACK must precede this transition. Retain the outcome until the
    /// separate full-image read and operation-specific publication succeed.
    pub fn begin_field_rename_reopen(&mut self, identity: FieldRenameSaveIdentity) -> bool {
        self.field_rename
            .status
            .as_mut()
            .is_some_and(|status| status.begin_reopen(identity))
    }

    pub fn fail_field_rename_reopen(
        &mut self,
        identity: FieldRenameSaveIdentity,
        message: String,
    ) -> bool {
        if !self
            .field_rename
            .status
            .as_ref()
            .is_some_and(|status| status.reopening(identity))
        {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_rename
            .status
            .as_mut()
            .unwrap()
            .record_reopen_failure(message, admitted);

        true
    }

    pub fn can_retry_field_rename_reopen(&self, identity: FieldRenameSaveIdentity) -> bool {
        self.field_rename
            .status
            .as_ref()
            .is_some_and(|status| status.can_retry_reopen(identity))
    }

    pub fn cancel_field_rename_operation(&mut self, identity: FieldRenameSaveIdentity) -> bool {
        self.field_rename
            .status
            .as_mut()
            .is_some_and(|status| status.cancel(identity))
    }

    /// Host precondition: the exact actor was acknowledged as original/no-write.
    /// Keep the proposed name and require a fresh native review for another Save.
    pub fn release_field_rename_save(
        &mut self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
    ) -> bool {
        if !self.matches_field_rename_save(request, identity)
            || !self.can_acknowledge_field_rename_save(identity)
        {
            return false;
        }

        self.field_rename.draft.as_mut().unwrap().phase = Phase::Naming;
        self.field_rename.status = None;

        true
    }

    /// Host precondition: complete FieldRenamedReopen read and rename-specific
    /// publication succeeded. An unchanged review still needs its own full read;
    /// it never manufactures a saved receipt or native save actor.
    pub fn clear_field_rename_save(
        &mut self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
    ) -> bool {
        if !self.matches_field_rename_save(request, identity) {
            return false;
        }

        self.field_rename = super::model::Model::default();

        true
    }
}
