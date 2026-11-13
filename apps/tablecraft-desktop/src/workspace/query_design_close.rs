//! Exact shared Design close decisions orchestrate existing build/save services.
use super::Desktop;
use tablecraft_ui::{QueryDesignCloseDecision, QueryDesignCloseResult, QueryDesignCloseSave};

impl Desktop {
    pub(super) fn finish_query_design_close(
        &mut self,
        result: QueryDesignCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            QueryDesignCloseDecision::Save => {
                match self.state.begin_query_design_close_save(&result.request) {
                    Some(QueryDesignCloseSave::Build {
                        request,
                        draft,
                        completion,
                    }) => {
                        if !self.build_query_design(request, draft, completion) {
                            self.state.cancel_query_design_close();
                            self.cancel_table_draft_action();
                        }
                    }
                    Some(QueryDesignCloseSave::Naming) => {}
                    None => self.cancel_table_draft_action(),
                }
            }
            QueryDesignCloseDecision::Discard => {
                // The shared owner has already applied this exact discard and
                // close. The host owns only the separately deferred transition.
                self.resume_table_draft_action(context);
            }
            QueryDesignCloseDecision::Cancel => self.cancel_table_draft_action(),
        }
    }
}
