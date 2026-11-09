//! Replace requests enter runtime admission before the existing reviewed edit UI.
use super::{Desktop, PendingOperation, editing, find};
use crate::workspace::request_read;
use tablecraft_runtime::{Command, EditDraft, FindPosition, FindSpan};
use tablecraft_ui::ReplaceFindRequest;

impl Desktop {
    pub(super) fn replace_find_match(&mut self, request: ReplaceFindRequest) {
        if self.editing.has_save() || self.state.busy || !self.state.can_replace_find(&request) {
            return;
        }

        let filter = match self.state.active_filter_target() {
            Some(target) => {
                let Some(handle) = self.filter_handle(&target) else {
                    self.state.error = Some(self.state.localizer.text("filtering-stale"));
                    return;
                };
                Some((handle, self.state.table_filter_enabled()))
            }
            None => None,
        };
        let command = Command::ReplaceFindMatch {
            request: find::runtime_request(request.find.clone()),
            found: FindPosition {
                parent_ordinal: request.found.parent_ordinal,
                column: request.found.column.clone(),
                span: request.found.span.as_ref().map(|span| FindSpan {
                    start_utf16: span.start_utf16,
                    end_utf16: span.end_utf16,
                }),
            },
            replacement: request.replacement.clone(),
            filter,
        };
        self.editing.clear_review();
        let Some(request_id) = request_read(command, &self.worker, &mut self.state) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_replace_find(request_id, request) {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::ReplaceFind { request_id };
        self.state.busy = true;
    }

    pub(super) fn publish_replace_find_draft(&mut self, generation: u64, draft: EditDraft) {
        let PendingOperation::ReplaceFind { request_id } = &self.pending else {
            return;
        };
        if generation != *request_id || !self.state.accepts_replace_find_reply(*request_id) {
            return;
        }
        let request_id = *request_id;
        if self
            .state
            .set_replace_find_draft(request_id, editing::ui_draft(draft))
        {
            self.pending = PendingOperation::Edit { request_id };
        } else {
            self.state
                .fail_replace_find(request_id, self.state.localizer.text("find-invalid-result"));
        }
    }
}

#[cfg(test)]
mod tests;
