//! Inert host inspection cannot replace a dirty or stale draft.
use super::draft::{EligibilityObservation, Phase};
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

    /// An acknowledged Created publication retains exact authored text; it
    /// still needs a real observation from the ordinary expression-edit service.
    pub fn saved_query_sql_needs_created_eligibility_inspection(&self) -> bool {
        self.saved_query.sql.as_ref().is_some_and(|draft| {
            draft.eligibility_observation == EligibilityObservation::CreatedUnobserved
        })
    }

    /// Inspect only ordinary editor eligibility for the exact Created baseline.
    /// Canonical inspection SQL is not replacement authoring text or a new epoch.
    pub fn begin_created_query_eligibility_inspection(
        &mut self,
        target: SavedQueryTarget,
        request_id: u64,
    ) -> Option<SavedQuerySqlRequest> {
        if request_id == 0
            || !self.saved_sql_target_active(&target)
            || self.saved_query.pending.is_some()
            || self.saved_query.prompt.is_some()
            || !self.saved_query.sql.as_ref().is_some_and(|draft| {
                draft.target == target
                    && draft.original.is_some()
                    && draft.eligibility_observation == EligibilityObservation::CreatedUnobserved
            })
        {
            return None;
        }
        self.begin_saved_sql_request(request_id, Phase::CreatedEligibilityInspection)
    }

    /// Only an exact request reply can publish an inert host eligibility fact.
    pub fn set_saved_query_sql_inspection(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        sql: Option<String>,
        eligibility: SavedQuerySqlEligibility,
    ) -> bool {
        let Some(draft) = self.saved_query.sql.as_ref() else {
            return false;
        };
        let Some(phase) = draft.pending.as_ref().map(|pending| pending.phase) else {
            return false;
        };
        if !matches!(
            phase,
            Phase::Inspection | Phase::CreatedEligibilityInspection
        ) || !self.saved_sql_target_active(&request.target)
            || !draft.accepts(request_id, request, phase)
        {
            return false;
        }
        if sql
            .as_ref()
            .is_some_and(|sql| sql.len() > MAX_DESIGN_TEXT_BYTES)
        {
            return false;
        }
        if phase == Phase::CreatedEligibilityInspection {
            let reuses_original = self.saved_query.sql.as_ref().unwrap().original == sql;
            let retained = if reuses_original {
                0
            } else {
                sql.as_ref().map_or(0, String::capacity)
            };
            if !self.admit_object_bytes(Some(retained)) {
                return false;
            }
            let draft = self.saved_query.sql.as_mut().unwrap();
            if draft.eligibility_observation != EligibilityObservation::CreatedUnobserved
                || (sql.is_none()
                    && eligibility == SavedQuerySqlEligibility::QualifiedExpressionSelect)
            {
                return false;
            }
            draft.eligibility = eligibility;
            // Move the actual stored observation; authored text has its own
            // lifetime and remains byte-for-byte unchanged.
            draft.eligibility_observation = if reuses_original {
                EligibilityObservation::CreatedObservedOriginal
            } else {
                EligibilityObservation::CreatedObserved(sql)
            };
            draft.pending = None;
            draft.error = None;
            return true;
        }
        let draft = self.saved_query.sql.as_ref().unwrap();
        let stored_baseline = match &draft.eligibility_observation {
            EligibilityObservation::CreatedObserved(sql) => sql,
            _ => &draft.original,
        };
        let created_observation = matches!(
            draft.eligibility_observation,
            EligibilityObservation::CreatedObservedOriginal
                | EligibilityObservation::CreatedObserved(_)
        );
        if (draft.dirty() || created_observation) && &sql != stored_baseline {
            let draft = self.saved_query.sql.as_mut().unwrap();
            draft.pending = None;
            draft.eligibility = SavedQuerySqlEligibility::Unavailable;
            draft.error = Some(SavedQuerySqlError::SourceChanged);
            return false;
        }
        if &sql == stored_baseline {
            let draft = self.saved_query.sql.as_mut().unwrap();
            draft.pending = None;
            draft.eligibility = eligibility;
            if !created_observation {
                draft.eligibility_observation = EligibilityObservation::Observed;
            }
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
        draft.eligibility_observation = EligibilityObservation::Observed;
        draft.pending = None;
        draft.error = None;
        true
    }
}
