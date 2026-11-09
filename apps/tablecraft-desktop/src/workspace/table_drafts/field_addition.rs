//! Defer transitions until the exact field-addition naming owner is explicitly resolved.
use super::{DeferredDraftAction, Desktop};
use tablecraft_ui::{FieldAdditionCloseDecision, FieldAdditionCloseResult};

impl Desktop {
    /// One existing deferred owner covers table, file and window transitions.
    /// This is called before opening another draft's confirmation; Field Save
    /// can change the source only after all unrelated dirty drafts are handled.
    pub(super) fn defer_field_addition_transition(
        &mut self,
        action: DeferredDraftAction,
    ) -> Option<DeferredDraftAction> {
        if !self.state.field_addition_open() {
            return Some(action);
        }
        let (tab, _, _) = self.state.field_addition_name()?;
        // NO_CLONE: field_addition_name borrows shared UI state; a close request mutates that owner
        // and must carry a stable independently owned exact tab identity.
        let tab = tab.clone();
        if self.state.request_field_addition_close(&tab) {
            self.deferred_draft_action = Some(action);
        }
        None
    }

    pub(in crate::workspace) fn finish_field_addition_close(
        &mut self,
        result: FieldAdditionCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            FieldAdditionCloseDecision::Cancel => self.cancel_table_draft_action(),
            FieldAdditionCloseDecision::Discard => {
                // Shared state already discarded only the confirmed exact draft.
                // The pending transition remains independently owned by the host.
                self.resume_table_draft_action(context);
            }
            FieldAdditionCloseDecision::Save => {
                let Some(request) = result.request else {
                    self.cancel_table_draft_action();
                    return;
                };
                if request.tab != result.tab
                    || request.draft_revision != result.draft_revision
                    || request.declaration != result.declaration
                {
                    self.cancel_table_draft_action();
                    return;
                }
                self.save_field_addition(request);
                self.restore_field_addition_close_after_refusal();
            }
        }
    }

    /// Review refusal or an acknowledged original image leaves the field name
    /// unsaved. Keep the transition intent and offer its explicit choices again.
    /// A retained native actor or failed AFTER read never enters this path.
    pub(in crate::workspace) fn restore_field_addition_close_after_refusal(&mut self) {
        if self.deferred_draft_action.is_none() || self.editing.has_save() || self.state.busy {
            return;
        }
        if let Some((tab, _, _)) = self.state.field_addition_name() {
            // NO_CLONE: Restoring close after refusal mutates shared UI while the borrowed field-
            // addition tab would otherwise alias it; the close owner must retain the
            // identity.
            let tab = tab.clone();
            self.state.request_field_addition_close(&tab);
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
