//! Inert host inspection cannot replace a dirty or stale draft.
use super::draft::Phase;
use super::{Draft, SavedQuerySqlEligibility, SavedQuerySqlError, SavedQuerySqlRequest};
use crate::workspace::{State, query::parameters::copy_text, saved_query::SavedQueryTarget};
use tablecraft_builders::query::MAX_DESIGN_TEXT_BYTES;

impl State {
    /// Bind a read inspection to exact object identity and current text revision.
    pub fn begin_saved_query_sql_inspection(
        &mut self,
        target: SavedQueryTarget,
        request_id: u64,
    ) -> Option<SavedQuerySqlRequest> {
        if !self.saved_sql_target_active(&target)
            || self.saved_query.pending.is_some()
            || self.saved_query.prompt.is_some()
        {
            return None;
        }
        if self
            .saved_query
            .sql
            .as_ref()
            .is_none_or(|draft| draft.target != target)
        {
            // Switching a target must park its owning SavedQueryState first.
            if self.saved_query.sql.as_ref().is_some_and(Draft::dirty) {
                return None;
            }
            if !self.admit_object_bytes(Some(size_of::<Draft>() + target.name.capacity())) {
                return None;
            }
            self.saved_query.sql = Some(Draft::new(target));
        }
        self.begin_saved_sql_request(request_id, Phase::Inspection)
    }

    /// Only an exact request reply can publish an inert host eligibility fact.
    pub fn set_saved_query_sql_inspection(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        sql: Option<String>,
        eligibility: SavedQuerySqlEligibility,
    ) -> bool {
        if !self.saved_sql_target_active(&request.target)
            || !self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.accepts(request_id, request, Phase::Inspection))
        {
            return false;
        }
        if sql
            .as_ref()
            .is_some_and(|sql| sql.len() > MAX_DESIGN_TEXT_BYTES)
        {
            return false;
        }
        let draft = self.saved_query.sql.as_ref().unwrap();
        if draft.dirty() && sql != draft.original {
            let draft = self.saved_query.sql.as_mut().unwrap();
            draft.pending = None;
            draft.eligibility = SavedQuerySqlEligibility::Unavailable;
            draft.error = Some(SavedQuerySqlError::SourceChanged);
            return false;
        }
        if draft.original == sql {
            let draft = self.saved_query.sql.as_mut().unwrap();
            draft.pending = None;
            draft.eligibility = eligibility;
            draft.error = None;
            return true;
        }
        let Some(next_revision) = draft.revision.checked_add(1) else {
            return false;
        };
        let Some(additional) = sql
            .as_ref()
            .map_or(Some(0), |sql| sql.capacity().checked_mul(2))
        else {
            return false;
        };
        if !self.admit_object_bytes(Some(additional)) {
            return false;
        }
        let proposed = match &sql {
            Some(sql) => match copy_text(sql) {
                Some(text) => text,
                None => return false,
            },
            None => String::new(),
        };
        let draft = self.saved_query.sql.as_mut().unwrap();
        draft.original = sql;
        draft.proposed = proposed;
        draft.revision = next_revision;
        draft.eligibility = eligibility;
        draft.pending = None;
        draft.error = None;
        true
    }
}
