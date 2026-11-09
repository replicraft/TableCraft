//! Retain exact tab/file/window intent while confirmed deletion remains unsaved.
use super::{DeferredDraftAction, Desktop};
use tablecraft_ui::{FieldDropCloseDecision, FieldDropCloseResult};

impl Desktop {
    pub(super) fn defer_field_drop_transition(
        &mut self,
        action: DeferredDraftAction,
    ) -> Option<DeferredDraftAction> {
        if !self.state.field_drop_open() {
            return Some(action);
        }
        if let DeferredDraftAction::CloseObject(tab) | DeferredDraftAction::ActivateObject(tab) =
            &action
            && !self.state.object_tab_is_current(tab)
        {
            return None;
        }
        if let Some(dismissed) = self.state.dismiss_field_drop_confirmation() {
            if let Some(request_id) = dismissed.inspection_request_id {
                self.cancel_field_drop_inspection(request_id);
            }
            return Some(action);
        }
        if self.state.request_current_field_drop_close() {
            self.deferred_draft_action = Some(action);
        }
        None
    }

    pub(in crate::workspace) fn finish_field_drop_close(
        &mut self,
        result: FieldDropCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            FieldDropCloseDecision::Cancel => self.cancel_table_draft_action(),
            FieldDropCloseDecision::Discard => self.resume_table_draft_action(context),
            FieldDropCloseDecision::Save => {
                let Some(request) = result.request else {
                    self.cancel_table_draft_action();
                    return;
                };
                if request.target != result.target
                    || request.draft_revision != result.draft_revision
                {
                    self.cancel_table_draft_action();
                    return;
                }
                self.save_field_drop(request);
                self.restore_field_drop_close_after_refusal();
            }
        }
    }

    /// Cancellation/original ACK preserves the confirmed pending mark and the
    /// actual deferred request. It never closes an uncommitted deletion owner.
    pub(in crate::workspace) fn restore_field_drop_close_after_refusal(&mut self) {
        if self.deferred_draft_action.is_some() && !self.editing.has_save() && !self.state.busy {
            self.state.request_current_field_drop_close();
        }
    }
}
