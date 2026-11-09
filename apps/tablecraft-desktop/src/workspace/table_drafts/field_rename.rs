//! Retain the original host transition while the exact rename owner is resolved.
use super::{DeferredDraftAction, Desktop};
use crate::workspace::PendingOperation;
use tablecraft_ui::{FieldRenameCloseDecision, FieldRenameCloseResult};

impl Desktop {
    pub(in crate::workspace) fn close_field_rename_editor(&mut self) {
        if let Some(prepared) = self.state.prepare_clean_field_rename_close() {
            let inspection = prepared.inspection_request_id();
            if inspection.is_some_and(|id| self.worker.publication_is_current(id)) {
                self.worker.cancel();
            }
            prepared.complete();
            self.finish_field_rename_inspection_cancellation(inspection);
        } else {
            self.state.request_current_field_rename_close();
        }
    }

    pub(super) fn defer_field_rename_transition(
        &mut self,
        action: DeferredDraftAction,
    ) -> Option<DeferredDraftAction> {
        if !self.state.field_rename_open() {
            return Some(action);
        }
        if let Some(prepared) = self.state.prepare_clean_field_rename_close() {
            let inspection = prepared.inspection_request_id();
            if inspection.is_some_and(|id| self.worker.publication_is_current(id)) {
                self.worker.cancel();
            }
            prepared.complete();
            self.finish_field_rename_inspection_cancellation(inspection);
            return Some(action);
        }
        if self.state.request_current_field_rename_close() {
            self.deferred_draft_action = Some(action);
        }
        None
    }

    pub(in crate::workspace) fn finish_field_rename_close(
        &mut self,
        result: FieldRenameCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            FieldRenameCloseDecision::Cancel => self.cancel_table_draft_action(),
            FieldRenameCloseDecision::Discard => {
                if result
                    .inspection_request_id
                    .is_some_and(|id| self.worker.publication_is_current(id))
                {
                    self.worker.cancel();
                }
                self.finish_field_rename_inspection_cancellation(result.inspection_request_id);
                self.resume_table_draft_action(context);
            }
            FieldRenameCloseDecision::Save => {
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
                self.save_field_rename(request);
                self.restore_field_rename_close_after_refusal();
            }
        }
    }

    fn finish_field_rename_inspection_cancellation(&mut self, inspection: Option<u64>) {
        if inspection.is_some_and(|id| self.generation == id
            && matches!(self.pending, PendingOperation::FieldRenameInspection { request_id } if request_id == id))
        {
            self.pending = PendingOperation::Read;
            self.state.busy = false;
        }
    }

    /// Readonly review refusal or acknowledged original retains the actual
    /// proposed name and deferred action; it never discards accepted input.
    pub(in crate::workspace) fn restore_field_rename_close_after_refusal(&mut self) {
        if self.deferred_draft_action.is_some() && !self.editing.has_save() && !self.state.busy {
            self.state.request_current_field_rename_close();
        }
    }
}
