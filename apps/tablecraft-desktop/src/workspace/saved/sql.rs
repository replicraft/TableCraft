//! Inert SQL inspection: runtime owns reconstruction and edit qualification.
use super::runtime_target;
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{Command, QuerySqlDraft, QuerySqlEligibility};
use tablecraft_ui::{
    SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult, SavedQuerySqlEligibility,
    SavedQueryTarget,
};

impl Desktop {
    pub(in crate::workspace) fn finish_saved_query_sql_close(
        &mut self,
        result: SavedQuerySqlCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            SavedQuerySqlCloseDecision::Discard => {
                self.close_object(result.tab);
                self.resume_table_draft_action(context);
            }
            SavedQuerySqlCloseDecision::Cancel => self.cancel_table_draft_action(),
            SavedQuerySqlCloseDecision::Save => {
                if let Some(request) = result.request {
                    self.save_query_definition_for_close(request);
                } else {
                    self.cancel_table_draft_action();
                }
            }
        }
    }

    pub(in crate::workspace) fn inspect_saved_query_sql(&mut self, target: SavedQueryTarget) {
        if self.editing.has_save() {
            return;
        }

        let request_id = self.worker.request(Command::InspectQuerySqlDraft {
            target: runtime_target(&target),
        });
        let Some(request) = self
            .state
            .begin_saved_query_sql_inspection(target, request_id)
        else {
            self.worker.cancel();
            return;
        };

        self.generation = request_id;
        self.pending = PendingOperation::SavedSqlInspection {
            request_id,
            request,
        };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_saved_query_sql(
        &mut self,
        request_id: u64,
        draft: QuerySqlDraft,
    ) {
        let PendingOperation::SavedSqlInspection {
            request_id: expected,
            request,
        } = &self.pending
        else {
            return;
        };
        if *expected != request_id || draft.target != runtime_target(&request.target) {
            return;
        }

        let eligibility = match draft.eligibility {
            QuerySqlEligibility::QualifiedConstantSelect => {
                SavedQuerySqlEligibility::QualifiedConstantSelect
            }
            QuerySqlEligibility::Unavailable => SavedQuerySqlEligibility::Unavailable,
        };
        self.state
            .set_saved_query_sql_inspection(request_id, request, draft.sql, eligibility);
    }
}

#[cfg(test)]
mod tests;
