//! Return from a failed full-image read without discarding accepted names or facts.
use super::{
    FieldRenameRequest, FieldRenameSaveIdentity, FieldRenameTarget,
    model::{Draft, Phase},
};
use crate::workspace::{
    EditSaveOutcome, SaveCleanupNotice, State,
    schema_save::{Status, StatusPhase},
};

pub(super) struct Warning {
    pub owner: Draft,
    pub status: Status<FieldRenameSaveIdentity>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub open: bool,
}

impl Warning {
    pub fn presented(&self) -> Option<&Draft> {
        self.open.then_some(&self.owner)
    }

    pub fn heap_bytes(&self) -> Option<usize> {
        self.owner
            .heap_bytes()?
            .checked_add(self.status.heap_bytes()?)
    }

    pub fn discard(&mut self, target: &FieldRenameTarget, revision: u64) -> bool {
        if !self.open || self.owner.target != *target || self.owner.revision != revision {
            return false;
        }

        self.open = false;

        true
    }
}

/// Historical status facts, never a new native save, replay or retry capability.
pub struct FieldRenameWarning<'a> {
    pub identity: FieldRenameSaveIdentity,
    pub target: &'a FieldRenameTarget,
    pub proposed_name: &'a str,
    pub outcome: Option<&'a EditSaveOutcome>,
    pub reopen_error: Option<&'a str>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub report_omitted: bool,
}

impl State {
    pub fn can_return_to_field_rename_draft(&self, identity: FieldRenameSaveIdentity) -> bool {
        self.field_rename.warning.is_none()
            && self.field_rename.status.as_ref().is_some_and(|status| {
                status.identity == identity
                    && status.phase == StatusPhase::ReopenFailed
                    && match identity {
                        FieldRenameSaveIdentity::NativeSave { .. } => {
                            status.outcome.as_ref().is_some_and(|outcome| {
                                !matches!(outcome, EditSaveOutcome::Retained { .. })
                            })
                        }
                        FieldRenameSaveIdentity::UnchangedReview { .. } => status.outcome.is_none(),
                    }
            })
    }

    /// Native branch prerequisite: actual actor ACK/release, known AFTER and a
    /// failed terminal full-image read. Unchanged branch prerequisite: genuine
    /// unchanged review consumed to its witness and that readonly read failed.
    /// Move names and status intact; no string copy/allocation occurs here.
    pub fn return_field_rename_to_draft(
        &mut self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
        cleanup: Option<SaveCleanupNotice>,
    ) -> bool {
        if matches!(identity, FieldRenameSaveIdentity::UnchangedReview { .. }) && cleanup.is_some()
            || !self.can_return_to_field_rename_draft(identity)
            || !self.matches_field_rename_save(request, identity)
        {
            return false;
        }

        let status = self.field_rename.status.take().unwrap();
        let mut owner = self.field_rename.draft.take().unwrap();
        owner.phase = Phase::Naming;
        self.field_rename.warning = Some(Warning {
            owner,
            status,
            cleanup,
            open: true,
        });

        true
    }

    pub fn field_rename_source_stale(&self) -> bool {
        self.field_rename.warning.as_ref().is_some_and(|warning| {
            self.database.as_ref().is_some_and(|database| {
                database.document_revision == warning.owner.target.tab.target.document_revision
            })
        })
    }

    pub fn field_rename_warning(&self) -> Option<FieldRenameWarning<'_>> {
        if !self.field_rename_source_stale() {
            return None;
        }

        let warning = self.field_rename.warning.as_ref()?;
        Some(FieldRenameWarning {
            identity: warning.status.identity,
            target: &warning.owner.target,
            proposed_name: &warning.owner.proposed,
            outcome: warning.status.outcome.as_ref(),
            reopen_error: warning.status.error.as_deref(),
            cleanup: warning.cleanup,
            report_omitted: warning.status.omitted,
        })
    }

    /// Only an explicit fresh document revision may retire closed history. An
    /// open proposal is never silently discarded by a general layout reset.
    pub(in crate::workspace) fn retire_field_rename_warning(&mut self, revision: Option<u64>) {
        if self
            .field_rename
            .selection
            .as_ref()
            .is_some_and(|target| Some(target.tab.target.document_revision) != revision)
        {
            self.field_rename.selection = None;
        }
        if self.field_rename.warning.as_ref().is_some_and(|warning| {
            !warning.open && Some(warning.owner.target.tab.target.document_revision) != revision
        }) {
            self.field_rename.warning = None;
        }
    }
}
