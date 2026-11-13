//! Source-neutral SQL ownership transfer after a host-proven query-only reopen.
use super::{Draft, SavedQuerySqlRequest, draft::Phase};
use crate::workspace::{
    objects::memory::Counter, query::parameters::copy_text, saved_query::SavedQueryState,
};

pub(in crate::workspace) struct SqlResolution {
    baseline: String,
    revision: u64,
}

impl SqlResolution {
    pub(in crate::workspace) fn prepare(request: &SavedQuerySqlRequest) -> Option<Self> {
        Some(Self {
            baseline: copy_text(&request.proposed_sql)?,
            revision: request.draft_revision.checked_add(1)?,
        })
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.baseline.capacity()
    }

    pub(in crate::workspace) fn baseline_capacity(&self) -> usize {
        self.baseline.capacity()
    }
}

pub(in crate::workspace) fn copy(text: &String) -> Option<String> {
    let mut result = String::new();
    result.try_reserve_exact(text.capacity()).ok()?;
    result.push_str(text);
    Some(result)
}

impl Draft {
    pub(in crate::workspace) fn matches_rebind_request(
        &self,
        request: &SavedQuerySqlRequest,
    ) -> bool {
        self.matches(request)
    }

    pub(in crate::workspace) fn accepts_rebind_review(
        &self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> bool {
        self.accepts(request_id, request, Phase::Reviewed(review_id))
    }

    #[cfg(test)]
    pub(in crate::workspace) fn rebind_buffer_identity(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize) {
        let original = self
            .original
            .as_ref()
            .map(|text| (text.as_ptr() as usize, text.capacity()))
            .unwrap_or((0, 0));
        (
            original.0,
            original.1,
            self.proposed.as_ptr() as usize,
            self.proposed.capacity(),
            self.target.name.as_ptr() as usize,
            self.target.name.capacity(),
        )
    }
}

impl SavedQueryState {
    /// Call only after every source/destination slot, identity and allocation has
    /// been validated. This publication step is infallible and moves existing
    /// raw buffers, preserving their exact capacities without a duplicate copy.
    pub(in crate::workspace) fn move_sql_rebind_to(
        &mut self,
        target: &mut Self,
        document_revision: u64,
        resolution: Option<SqlResolution>,
    ) {
        let accepted_text_revision = resolution
            .as_ref()
            .and_then(|resolution| resolution.revision.checked_sub(1));
        let resolved = resolution.is_some();
        target.sql = self.sql.take();
        if let Some(sql) = &mut target.sql {
            sql.target.document_revision = document_revision;
            sql.pending = None;
            if let Some(resolution) = resolution {
                sql.original = Some(resolution.baseline);
                sql.saved_baseline_replaced();
                sql.revision = resolution.revision;
                sql.error = None;
            }
        }
        target.error = if resolved { None } else { self.error.take() };
        if let Some(revision) = accepted_text_revision {
            target.design.resolve_saved_text(revision);
        }
    }

    pub(in crate::workspace) fn moved_sql_payload_bytes(
        &self,
        baseline: Option<usize>,
    ) -> Option<usize> {
        let mut count = Counter::new(0);
        if let Some(sql) = &self.sql {
            count.add(size_of::<Draft>());
            count.string(&sql.target.name);
            if let Some(capacity) = baseline {
                count.add(capacity);
            } else {
                count.optional(&sql.original);
                if let Some(super::SavedQuerySqlError::Host(error)) = &sql.error {
                    count.string(error);
                }
            }
            if baseline.is_none()
                && let super::draft::EligibilityObservation::CreatedObserved(observed) =
                    &sql.eligibility_observation
            {
                count.optional(observed);
            }
            count.string(&sql.proposed);
        } else if baseline.is_some() {
            return None;
        }
        if baseline.is_none() {
            count.optional(&self.error);
        }
        count.finish()
    }

    /// Bound only the parameter preference copy; cached rows and raw SQL move
    /// through separate ownership paths and are not cloned by layout capture.
    pub(in crate::workspace) fn sql_preference_copy_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<super::super::InputPreferences>());
        let inputs = if let Some(restored) = &self.restored_inputs {
            count.vector(&restored.declarations);
            for (name, kind) in &restored.declarations {
                count.string(name);
                count.string(kind);
            }
            &restored.inputs
        } else if let Some(parameters) = &self.parameters {
            count.count::<(String, String)>(parameters.len());
            for parameter in parameters {
                count.string(&parameter.name);
                count.string(&parameter.kind);
            }
            &self.inputs
        } else {
            return Some(0);
        };
        count.vector(inputs);
        for input in inputs {
            count.string(&input.name);
            count.string(&input.kind);
            count.string(&input.text);
        }
        count.finish()
    }
}
