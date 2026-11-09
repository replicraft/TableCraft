use super::draft::{Pending, Phase};
use super::{Draft, SavedQuerySqlEligibility, SavedQuerySqlError, SavedQuerySqlRequest};
use crate::workspace::{State, saved_query::SavedQueryTarget};
use tablecraft_builders::query::MAX_DESIGN_TEXT_BYTES;

impl State {
    pub(super) fn saved_sql_target_active(&self, target: &SavedQueryTarget) -> bool {
        self.saved_query_visible()
            && self.saved_query_binding(target)
            && self.saved_query.target.as_ref() == Some(target)
    }

    /// Read-only observation and text for the selected saved object.
    pub fn saved_query_sql(&self) -> Option<(&str, &str, u64, SavedQuerySqlEligibility)> {
        let draft = self.saved_query.sql.as_ref()?;
        Some((
            draft.original.as_deref()?,
            &draft.proposed,
            draft.revision,
            draft.eligibility,
        ))
    }

    pub fn saved_query_sql_pending(&self) -> bool {
        self.saved_query
            .sql
            .as_ref()
            .is_some_and(|draft| draft.pending.is_some())
    }

    pub fn saved_query_sql_eligibility(&self) -> Option<SavedQuerySqlEligibility> {
        Some(self.saved_query.sql.as_ref()?.eligibility)
    }

    pub fn saved_query_sql_error(&self) -> Option<&SavedQuerySqlError> {
        self.saved_query.sql.as_ref()?.error.as_ref()
    }

    pub fn saved_query_sql_dirty(&self) -> bool {
        self.saved_query.sql.as_ref().is_some_and(Draft::dirty)
    }

    pub fn can_edit_saved_query_sql(&self) -> bool {
        !self.native_mutation_blocked()
            && self.saved_query_sql_edit_available
            && self.object_switch_ready()
            && !self.has_unsaved_table_drafts()
            && self.saved_query.pending.is_none()
            && self.saved_query.prompt.is_none()
            && self.saved_query.sql.as_ref().is_some_and(|draft| {
                self.saved_sql_target_active(&draft.target)
                    && draft.original.is_some()
                    && draft.eligibility == SavedQuerySqlEligibility::QualifiedExpressionSelect
            })
    }

    /// Own exact user text without parsing, formatting or canonicalizing it.
    pub fn set_saved_query_sql_text(&mut self, text: String) -> bool {
        if !self.can_edit_saved_query_sql()
            || text.len() > MAX_DESIGN_TEXT_BYTES
            || !self.admit_object_bytes(Some(text.capacity()))
        {
            return false;
        }
        self.saved_query.sql.as_mut().unwrap().replace_text(text)
    }

    pub fn can_review_saved_query_sql(&self) -> bool {
        self.can_edit_saved_query_sql()
            && self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.dirty() && draft.pending.is_none())
    }

    /// Bounded outgoing Action snapshot, without assigning a host request ID.
    pub fn saved_query_sql_review_request(&self) -> Option<SavedQuerySqlRequest> {
        if !self.can_review_saved_query_sql() {
            return None;
        }
        let request = self.saved_query.sql.as_ref()?.snapshot()?;
        if self
            .object_bytes()?
            .checked_add(request.retained_bytes()?)?
            > crate::workspace::objects::MAX_RETAINED_BYTES
        {
            return None;
        }
        Some(request)
    }

    /// Exact identity/text guard for a host immediately before dispatch.
    pub fn saved_query_sql_request_current(&self, request: &SavedQuerySqlRequest) -> bool {
        self.saved_sql_target_active(&request.target)
            && self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.matches(request))
    }

    /// Retain the complete original/proposed SQL until native review finishes.
    pub fn begin_saved_query_sql_review(
        &mut self,
        request_id: u64,
    ) -> Option<SavedQuerySqlRequest> {
        if !self.can_review_saved_query_sql() {
            return None;
        }
        self.begin_saved_sql_request(request_id, Phase::Review)
    }

    pub fn set_saved_query_sql_review(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> bool {
        if !self.saved_sql_target_active(&request.target)
            || !self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.accepts(request_id, request, Phase::Review))
        {
            return false;
        }
        let draft = self.saved_query.sql.as_mut().unwrap();
        draft.pending.as_mut().unwrap().phase = Phase::Reviewed(review_id);
        draft.error = None;
        true
    }

    /// Presentation matching only: a host must still consume its native capability.
    pub fn matches_saved_query_sql_review(
        &self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> bool {
        self.saved_sql_target_active(&request.target)
            && self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.accepts(request_id, request, Phase::Reviewed(review_id)))
    }

    pub fn fail_saved_query_sql(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        message: String,
    ) -> bool {
        if !self.saved_sql_target_active(&request.target)
            || !self.saved_query.sql.as_ref().is_some_and(|draft| {
                draft.matches(request)
                    && draft.pending.as_ref().is_some_and(|pending| {
                        pending.request_id == request_id && pending.snapshot == *request
                    })
            })
        {
            return false;
        }
        // Reporting failure must not strand an unlocked review. Preserve its
        // exact draft and publish a bounded diagnostic when the payload cannot
        // fit; only this matching pending request is released.
        let error = if self.admit_object_bytes(Some(message.capacity())) {
            SavedQuerySqlError::Host(message)
        } else {
            SavedQuerySqlError::MemoryLimit
        };
        let draft = self.saved_query.sql.as_mut().unwrap();
        draft.pending = None;
        draft.error = Some(error);
        true
    }

    pub(super) fn begin_saved_sql_request(
        &mut self,
        request_id: u64,
        phase: Phase,
    ) -> Option<SavedQuerySqlRequest> {
        let draft = self.saved_query.sql.as_ref()?;
        if draft.pending.is_some() {
            return None;
        }
        let returned = draft.snapshot()?;
        let retained = draft.snapshot()?;
        // Include both the parked pending copy and the outgoing owned request.
        if !self.admit_object_bytes(
            retained
                .retained_bytes()?
                .checked_add(returned.retained_bytes()?),
        ) {
            return None;
        }
        let draft = self.saved_query.sql.as_mut().unwrap();
        draft.pending = Some(Pending {
            request_id,
            snapshot: retained,
            phase,
        });
        draft.error = None;
        Some(returned)
    }
}
