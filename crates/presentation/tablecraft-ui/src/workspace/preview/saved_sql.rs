//! Synthetic query Save never issues native authority or reports persistence.
use super::Preview;
use crate::{SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult, SavedQuerySqlRequest};

impl Preview {
    pub(super) fn refuse_saved_sql_save(&mut self, request: SavedQuerySqlRequest) {
        if !self.state.saved_query_sql_request_current(&request) {
            return;
        }
        let id = self.request();
        let Some(snapshot) = self.state.begin_saved_query_sql_review(id) else {
            return;
        };
        self.state.fail_saved_query_sql(
            id,
            &snapshot,
            self.state.localizer.text("ribbon-unavailable"),
        );
    }

    pub(super) fn finish_saved_sql_close(&mut self, result: SavedQuerySqlCloseResult) {
        match result.decision {
            SavedQuerySqlCloseDecision::Save => {
                if let Some(request) = result.request {
                    self.refuse_saved_sql_save(request);
                }
            }
            SavedQuerySqlCloseDecision::Discard => {
                // The shared confirmation already discarded its exact revision.
                // A delayed event cannot close a subsequently edited object.
                if self
                    .state
                    .saved_query_sql_revision_for(&result.tab)
                    .is_none()
                {
                    self.state.close_object(&result.tab);
                }
            }
            SavedQuerySqlCloseDecision::Cancel => {}
        }
    }
}

#[cfg(test)]
mod tests;
