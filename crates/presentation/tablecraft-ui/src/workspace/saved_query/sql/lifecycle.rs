//! Presentation completion is separate from native save/reopen authority.
use super::{SavedQuerySqlError, SavedQuerySqlRequest, draft::Phase};
use crate::workspace::{State, query::parameters::copy_text};

impl State {
    fn saved_sql_accepted_review_current(
        &self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> bool {
        self.saved_query_binding(&request.target)
            && self
                .saved_sql_lifecycle_state(&request.target)
                .and_then(|state| state.sql.as_ref())
                .is_some_and(|draft| draft.accepts(request_id, request, Phase::Reviewed(review_id)))
    }

    /// Release/fail an exact accepted review after an original/no-write outcome.
    /// This changes presentation pending/error state only, preserving all text,
    /// the original baseline, dirty state and text revision. It cannot release
    /// native resources or certify a persisted outcome.
    pub fn release_saved_query_sql_review(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
        message: Option<String>,
    ) -> bool {
        if !self.saved_sql_accepted_review_current(request_id, request, review_id)
            || !self.admit_object_bytes(Some(message.as_ref().map_or(0, String::capacity)))
        {
            return false;
        }
        let Some(draft) = self
            .saved_sql_lifecycle_state_mut(&request.target)
            .and_then(|state| state.sql.as_mut())
        else {
            return false;
        };
        draft.pending = None;
        draft.error = message.map(SavedQuerySqlError::Host);
        self.refresh_saved_sql_lifecycle_memory(&request.target);
        true
    }

    /// Resolve the exact accepted request AFTER the host acknowledges a known
    /// replacement and successfully performs the complete-witness fresh reopen.
    ///
    /// This API supplies no backend authority or outcome proof. The host owns
    /// those preconditions and must retain/rebind this exact visible text when
    /// publishing the fresh catalog. No review, receipt alone or failed reopen
    /// may call this method. The submitted spelling remains unchanged.
    pub fn resolve_saved_query_sql_review(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> bool {
        if !self.saved_sql_accepted_review_current(request_id, request, review_id) {
            return false;
        }
        let Some(revision) = request.draft_revision.checked_add(1) else {
            return false;
        };
        let Some(original) = copy_text(&request.proposed_sql) else {
            return false;
        };
        if !self.admit_object_bytes(Some(original.capacity())) {
            return false;
        }
        let Some(draft) = self
            .saved_sql_lifecycle_state_mut(&request.target)
            .and_then(|state| state.sql.as_mut())
        else {
            return false;
        };
        draft.original = Some(original);
        draft.revision = revision;
        draft.pending = None;
        draft.error = None;
        self.refresh_saved_sql_lifecycle_memory(&request.target);
        true
    }
}

#[cfg(test)]
mod tests;
