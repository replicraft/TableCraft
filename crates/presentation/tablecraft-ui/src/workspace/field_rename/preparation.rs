//! Exclusive, pre-admitted presentation ownership before consuming runtime authority.
use super::{
    FieldRenameRequest, FieldRenameSaveIdentity,
    model::{Draft, Phase},
};
use crate::workspace::{
    State,
    schema_save::{Status, StatusPhase},
};

struct Preparation<'a> {
    draft: &'a mut Draft,
    status: &'a mut Option<Status<FieldRenameSaveIdentity>>,
    request_id: u64,
    review_id: u64,
    bound: bool,
}

/// The exact changed UI request and empty inline status are borrowed before
/// the host starts a native actor. This token grants no native write authority.
pub struct PreparedFieldRenameSave<'a> {
    prepared: Preparation<'a>,
}

/// The exact unchanged UI request is borrowed before the actual runtime
/// capability is consumed into its complete-image read-only witness.
pub struct PreparedUnchangedFieldRename<'a> {
    prepared: Preparation<'a>,
}

impl PreparedFieldRenameSave<'_> {
    /// Bind only the real accepted SaveWorker ID. No allocation or lookup follows.
    pub fn bind(mut self, save_id: u64) -> FieldRenameSaveIdentity {
        let identity = FieldRenameSaveIdentity::NativeSave {
            request_id: self.prepared.request_id,
            review_id: self.prepared.review_id,
            save_id,
        };
        self.prepared.install(
            identity,
            StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
        );
        identity
    }
}

impl PreparedUnchangedFieldRename<'_> {
    /// Host prerequisite: a genuine unchanged runtime review was consumed into
    /// its full-image witness. No native save actor, ID or receipt is invented.
    pub fn bind(mut self) -> FieldRenameSaveIdentity {
        let identity = FieldRenameSaveIdentity::UnchangedReview {
            request_id: self.prepared.request_id,
            review_id: self.prepared.review_id,
        };
        self.prepared
            .install(identity, StatusPhase::Reopening { cancelled: false });
        identity
    }
}

impl Preparation<'_> {
    fn install(&mut self, identity: FieldRenameSaveIdentity, phase: StatusPhase) {
        self.draft.phase = Phase::Owned { identity };
        *self.status = Some(Status {
            identity,
            phase,
            outcome: None,
            error: None,
            omitted: false,
        });
        self.bound = true;
    }
}

impl Drop for Preparation<'_> {
    fn drop(&mut self) {
        if !self.bound {
            self.draft.phase = Phase::Naming;
        }
    }
}

impl State {
    pub fn prepare_field_rename_save(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> Option<PreparedFieldRenameSave<'_>> {
        Some(PreparedFieldRenameSave {
            prepared: self
                .prepare_field_rename_presentation(request, request_id, review_id, true)?,
        })
    }

    pub fn prepare_unchanged_field_rename(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> Option<PreparedUnchangedFieldRename<'_>> {
        Some(PreparedUnchangedFieldRename {
            prepared: self
                .prepare_field_rename_presentation(request, request_id, review_id, false)?,
        })
    }

    fn prepare_field_rename_presentation(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
        changed: bool,
    ) -> Option<Preparation<'_>> {
        if (request.new_name != request.target.column) != changed
            || self.field_rename.status.is_some()
            || !self.field_rename_owner_bound(&request.target)
            || !self.field_rename.matches(
                request,
                Phase::Reviewed {
                    request_id,
                    review_id,
                },
            )
            || self.has_unsaved_table_drafts()
            || self.has_modified_saved_query_sql()
            || self.field_addition_open()
            || self.database_options_pending()
        {
            return None;
        }

        Some(Preparation {
            draft: self.field_rename.draft.as_mut()?,
            status: &mut self.field_rename.status,
            request_id,
            review_id,
            bound: false,
        })
    }
}

/// Borrow the exact prepared source slot before starting the readonly task.
/// Bind the real worker ID without allocation or fallible lookup afterwards.
pub struct PreparedFieldRenameInspection<'a> {
    observation: &'a mut super::model::Observation,
    bound: bool,
}

impl PreparedFieldRenameInspection<'_> {
    pub fn bind(mut self, request_id: u64) {
        *self.observation = super::model::Observation::Pending {
            request_id: Some(request_id),
        };
        self.bound = true;
    }
}

impl Drop for PreparedFieldRenameInspection<'_> {
    fn drop(&mut self) {
        if !self.bound {
            *self.observation = super::model::Observation::Interrupted;
        }
    }
}

impl State {
    pub fn prepare_field_rename_inspection(
        &mut self,
        target: &super::FieldRenameTarget,
    ) -> Option<PreparedFieldRenameInspection<'_>> {
        if !self.field_rename_owner_bound(target) {
            return None;
        }
        let draft = self.field_rename.draft.as_mut()?;
        if draft.target != *target
            || !matches!(
                draft.observation,
                super::model::Observation::Pending { request_id: None }
            )
        {
            return None;
        }

        Some(PreparedFieldRenameInspection {
            observation: &mut draft.observation,
            bound: false,
        })
    }
}

/// Own the exact terminal presentation before asking the real actor to recover
/// or release. A refused worker start drops this token without changing status.
pub struct PreparedFieldRenameResolution<'a> {
    draft: &'a mut Draft,
    status: &'a mut Status<FieldRenameSaveIdentity>,
    request_id: u64,
    review_id: u64,
}

impl PreparedFieldRenameResolution<'_> {
    pub fn bind(self, actual_next_id: u64) -> FieldRenameSaveIdentity {
        let identity = FieldRenameSaveIdentity::NativeSave {
            request_id: self.request_id,
            review_id: self.review_id,
            save_id: actual_next_id,
        };
        self.draft.phase = Phase::Owned { identity };
        self.status.identity = identity;
        self.status.phase = StatusPhase::Saving {
            cancelled: false,
            resolving: true,
        };
        self.status.outcome = None;
        identity
    }
}

impl State {
    pub fn prepare_field_rename_resolution(
        &mut self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
        recover: bool,
    ) -> Option<PreparedFieldRenameResolution<'_>> {
        let FieldRenameSaveIdentity::NativeSave {
            request_id,
            review_id,
            ..
        } = identity
        else {
            return None;
        };
        if !self.matches_field_rename_save(request, identity)
            || !self.can_resolve_field_rename_save(identity, recover)
        {
            return None;
        }

        Some(PreparedFieldRenameResolution {
            draft: self.field_rename.draft.as_mut()?,
            status: self.field_rename.status.as_mut()?,
            request_id,
            review_id,
        })
    }
}
