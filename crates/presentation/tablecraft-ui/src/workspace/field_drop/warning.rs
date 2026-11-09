use super::{
    FieldDropRequest, FieldDropSaveIdentity, FieldDropTarget,
    model::{Draft, Phase},
};
use crate::workspace::{
    EditSaveOutcome, SaveCleanupNotice, State,
    schema_save::{Status, StatusPhase},
};

pub(super) struct Warning {
    pub owner: Draft,
    pub status: Status<FieldDropSaveIdentity>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub open: bool,
}

/// Retained historical outcome after a failed full-image publication read.
/// This never means the dropped field was rolled back or that retrying is safe.
pub struct FieldDropWarning<'a> {
    pub identity: FieldDropSaveIdentity,
    pub target: &'a FieldDropTarget,
    pub outcome: Option<&'a EditSaveOutcome>,
    pub reopen_error: Option<&'a str>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub report_omitted: bool,
}

impl State {
    pub fn can_return_to_field_drop_draft(&self, identity: FieldDropSaveIdentity) -> bool {
        self.field_drop.warning.is_none()
            && self.field_drop.status.as_ref().is_some_and(|status| {
                status.identity == identity
                    && status.phase == StatusPhase::ReopenFailed
                    && status
                        .outcome
                        .as_ref()
                        .is_some_and(|outcome| !matches!(outcome, EditSaveOutcome::Retained { .. }))
            })
    }

    /// Host prerequisite: real known AFTER has been ACKed/released and the exact
    /// full-image read failed. Move all status/text/cleanup without allocation.
    pub fn return_field_drop_to_draft(
        &mut self,
        request: &FieldDropRequest,
        identity: FieldDropSaveIdentity,
        cleanup: Option<SaveCleanupNotice>,
    ) -> bool {
        if !self.can_return_to_field_drop_draft(identity)
            || !self.matches_field_drop_save(request, identity)
        {
            return false;
        }
        let status = self.field_drop.status.take().unwrap();
        let mut owner = self.field_drop.draft.take().unwrap();
        owner.phase = Phase::PendingDeletion;
        self.field_drop.warning = Some(Warning {
            owner,
            status,
            cleanup,
            open: true,
        });
        true
    }

    pub fn field_drop_source_stale(&self) -> bool {
        self.field_drop.warning.as_ref().is_some_and(|warning| {
            self.database.as_ref().is_some_and(|database| {
                database.document_revision == warning.owner.target.tab.target.document_revision
            })
        })
    }

    pub fn field_drop_warning(&self) -> Option<FieldDropWarning<'_>> {
        if !self.field_drop_source_stale() {
            return None;
        }
        let warning = self.field_drop.warning.as_ref()?;
        Some(FieldDropWarning {
            identity: warning.status.identity,
            target: &warning.owner.target,
            outcome: warning.status.outcome.as_ref(),
            reopen_error: warning.status.error.as_deref(),
            cleanup: warning.cleanup,
            report_omitted: warning.status.omitted,
        })
    }

    pub(in crate::workspace) fn retire_field_drop_warning(&mut self, revision: Option<u64>) {
        if self.field_drop.warning.as_ref().is_some_and(|warning| {
            !warning.open && Some(warning.owner.target.tab.target.document_revision) != revision
        }) {
            self.field_drop.warning = None;
        }
    }
}
