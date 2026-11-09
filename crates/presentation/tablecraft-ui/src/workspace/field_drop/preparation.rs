use super::{
    FieldDropRequest, FieldDropSaveIdentity, FieldDropTarget,
    model::{Observation, Phase},
};
use crate::workspace::{
    State,
    schema_save::{Preparation, Status, StatusPhase},
};

/// Exclusive presentation admission before the actual save actor accepts.
/// This is not a native write capability and performs no disk operation.
pub struct PreparedFieldDropSave<'a> {
    prepared: Preparation<'a, FieldDropSaveIdentity, Phase>,
    request_id: u64,
    review_id: u64,
}

impl PreparedFieldDropSave<'_> {
    pub fn bind(mut self, save_id: u64) -> FieldDropSaveIdentity {
        let identity = FieldDropSaveIdentity {
            request_id: self.request_id,
            review_id: self.review_id,
            save_id,
        };
        self.prepared.install(
            identity,
            Phase::Owned { identity },
            StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
        );
        identity
    }
}

/// Own the exact readonly observation slot before starting its real worker.
pub struct PreparedFieldDropInspection<'a> {
    observation: &'a mut Observation,
    bound: bool,
}

impl PreparedFieldDropInspection<'_> {
    pub fn bind(mut self, request_id: u64) {
        *self.observation = Observation::Pending {
            request_id: Some(request_id),
        };
        self.bound = true;
    }
}

impl Drop for PreparedFieldDropInspection<'_> {
    fn drop(&mut self) {
        if !self.bound {
            *self.observation = Observation::Interrupted;
        }
    }
}

/// Borrow a retained terminal owner before the actual recover/release actor starts.
pub struct PreparedFieldDropResolution<'a> {
    phase: &'a mut Phase,
    status: &'a mut Status<FieldDropSaveIdentity>,
    identity: FieldDropSaveIdentity,
}

impl PreparedFieldDropResolution<'_> {
    pub fn bind(self, save_id: u64) -> FieldDropSaveIdentity {
        let identity = FieldDropSaveIdentity {
            save_id,
            ..self.identity
        };
        *self.phase = Phase::Owned { identity };
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
    pub fn prepare_field_drop_inspection(
        &mut self,
        target: &FieldDropTarget,
    ) -> Option<PreparedFieldDropInspection<'_>> {
        if !self.field_drop_context(target) {
            return None;
        }
        let draft = self.field_drop.draft.as_mut()?;
        if draft.target != *target
            || draft.phase != Phase::Confirming
            || !matches!(draft.observation, Observation::Pending { request_id: None })
        {
            return None;
        }
        Some(PreparedFieldDropInspection {
            observation: &mut draft.observation,
            bound: false,
        })
    }

    pub fn prepare_field_drop_save(
        &mut self,
        request: &FieldDropRequest,
        request_id: u64,
        review_id: u64,
    ) -> Option<PreparedFieldDropSave<'_>> {
        if self.field_drop.status.is_some()
            || !self.field_drop_owner_bound(&request.target)
            || !self.field_drop.matches(
                request,
                Phase::Reviewed {
                    request_id,
                    review_id,
                },
            )
            || self.has_unsaved_table_drafts()
            || self.has_modified_saved_query_sql()
            || self.field_addition_open()
            || self.field_rename_open()
            || self.database_options_pending()
        {
            return None;
        }
        Some(PreparedFieldDropSave {
            prepared: Preparation::new(
                &mut self.field_drop.draft.as_mut()?.phase,
                &mut self.field_drop.status,
                Phase::PendingDeletion,
            ),
            request_id,
            review_id,
        })
    }

    pub fn prepare_field_drop_resolution(
        &mut self,
        request: &FieldDropRequest,
        identity: FieldDropSaveIdentity,
        recover: bool,
    ) -> Option<PreparedFieldDropResolution<'_>> {
        if !self.matches_field_drop_save(request, identity)
            || !self.can_resolve_field_drop_save(identity, recover)
        {
            return None;
        }
        Some(PreparedFieldDropResolution {
            phase: &mut self.field_drop.draft.as_mut()?.phase,
            status: self.field_drop.status.as_mut()?,
            identity,
        })
    }
}
