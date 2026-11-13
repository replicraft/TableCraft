//! Design Run shares issued parameter acquisition while retaining its draft origin.
use super::{PendingKind, QueryDesignRequest};
use crate::workspace::State;

impl State {
    pub(in crate::workspace) fn query_design_parameter_origin(&self) -> Option<QueryDesignRequest> {
        if self.query_design.has_unresolved_field_inputs()
            || !self.query_design_owned_results_visible()
            || !self.query_design_ready()
        {
            return None;
        }
        self.query_design_request()
    }

    pub(in crate::workspace) fn query_design_parameter_origin_current(
        &self,
        origin: &QueryDesignRequest,
    ) -> bool {
        !self.query_design.has_unresolved_field_inputs()
            && self.query_design_owned_results_visible()
            && self.query_design_request().as_ref() == Some(origin)
    }

    pub(in crate::workspace) fn begin_query_design_parameter_run(
        &mut self,
        request_id: u64,
        sql: String,
    ) -> bool {
        if request_id == 0
            || self.query_design_sql() != Some(sql.as_str())
            || !self.admit_object_bytes(Some(sql.capacity()))
        {
            return false;
        }
        let Some(request) = self.query_design_parameter_origin() else {
            return false;
        };
        self.begin_design_pending(request, request_id, PendingKind::Run { sql });
        true
    }

    pub(in crate::workspace) fn query_design_run_request_id(&self) -> Option<u64> {
        let pending = self.query_design.session.as_ref()?.pending.as_ref()?;
        matches!(pending.kind, PendingKind::Run { .. }).then_some(pending.id)
    }

    pub(in crate::workspace) fn clear_query_design_parameter_run(&mut self) {
        if self.query_design_run_request_id().is_some()
            && let Some(session) = self.query_design.session.as_mut()
        {
            session.pending = None;
        }
    }
}

#[cfg(test)]
mod tests;
