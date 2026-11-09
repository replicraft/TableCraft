//! Exclusive presentation admission before a native actor can be accepted.
use super::{FieldAdditionSaveIdentity, Status, StatusPhase};
use crate::workspace::{
    State,
    field_addition::{Draft, FieldAdditionRequest, Phase},
};

/// Holds the exact named review and its inline status slot exclusively. The host
/// may start a worker through its disjoint runtime field while this borrow lives;
/// no callback or widget can change the presentation state before binding.
/// This token is presentation ownership, never a native write capability.
pub struct PreparedFieldAdditionSave<'a> {
    draft: &'a mut Draft,
    status: &'a mut Option<Status>,
    request_id: u64,
    review_id: u64,
    bound: bool,
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
        self.draft.phase = Phase::Saving {
            request_id: identity.request_id,
            review_id: identity.review_id,
            save_id,
        };
        *self.status = Some(Status {
            identity,
            phase: StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
            outcome: None,
            error: None,
            omitted: false,
        });
        self.bound = true;
        identity
    }
}

impl Drop for PreparedFieldAdditionSave<'_> {
    fn drop(&mut self) {
        if !self.bound {
            // A refused worker returns its unconsumed capability separately.
            // Dropping this token cannot erase the user's proposed field name.
            self.draft.phase = Phase::Naming;
        }
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
            draft: self.field_addition.draft.as_mut()?,
            status: &mut self.field_addition_status,
            request_id,
            review_id,
            bound: false,
        })
    }
}

#[cfg(test)]
mod tests;
