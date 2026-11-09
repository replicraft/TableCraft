//! Exclusive presentation admission before a native actor can be accepted.
use super::FieldAdditionSaveIdentity;
use crate::workspace::schema_save::{Preparation, StatusPhase};
use crate::workspace::{
    State,
    field_addition::{FieldAdditionRequest, Phase},
};

/// Holds the exact named review and its inline status slot exclusively. The host
/// may start a worker through its disjoint runtime field while this borrow lives;
/// no callback or widget can change the presentation state before binding.
/// This token is presentation ownership, never a native write capability.
pub struct PreparedFieldAdditionSave<'a> {
    prepared: Preparation<'a, FieldAdditionSaveIdentity, Phase>,
    request_id: u64,
    review_id: u64,
}

impl PreparedFieldAdditionSave<'_> {
    /// Consume the preparation after the real worker accepts. No allocation,
    /// admission, identity lookup or fallible state transition remains here.
    pub fn bind(mut self, save_id: u64) -> FieldAdditionSaveIdentity {
        let identity = FieldAdditionSaveIdentity {
            request_id: self.request_id,
            review_id: self.review_id,
            save_id,
        };
        self.prepared.install(
            identity,
            Phase::Saving {
                request_id: identity.request_id,
                review_id: identity.review_id,
                save_id,
            },
            StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
        );
        identity
    }
}

impl State {
    /// Borrow the exact accepted review and empty status slot before starting
    /// a native worker. Failed preparation leaves state and native authority intact.
    pub fn prepare_field_addition_save(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
    ) -> Option<PreparedFieldAdditionSave<'_>> {
        if self.field_addition_status.is_some()
            || !self.field_addition_binding(&request.tab)
            || !self.field_addition.matches(
                request,
                Phase::Reviewed {
                    request_id,
                    review_id,
                },
            )
            || self.has_unsaved_table_drafts()
            || self.has_modified_saved_query_sql()
        {
            return None;
        }
        Some(PreparedFieldAdditionSave {
            prepared: Preparation::new(
                &mut self.field_addition.draft.as_mut()?.phase,
                &mut self.field_addition_status,
                Phase::Naming,
            ),
            request_id,
            review_id,
        })
    }
}

#[cfg(test)]
mod tests;
