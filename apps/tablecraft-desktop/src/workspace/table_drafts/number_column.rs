//! Defer transitions until the exact Number naming owner is explicitly resolved.
use super::{DeferredDraftAction, Desktop};
use tablecraft_ui::{NumberColumnCloseDecision, NumberColumnCloseResult, ObjectTab};

impl Desktop {
    /// One existing deferred owner covers table, file and window transitions.
    /// This is called before opening another draft's confirmation; Number Save
    /// can change the source only after all unrelated dirty drafts are handled.
    pub(super) fn defer_number_column_transition(
        &mut self,
        action: DeferredDraftAction,
    ) -> Option<DeferredDraftAction> {
        if !self.state.number_column_open() {
            return Some(action);
        }
        let (tab, _, _) = self.state.number_column_name()?;
        let tab = tab.clone();
        if self.state.request_number_column_close(&tab) {
            self.deferred_draft_action = Some(action);
        }
        None
    }

    pub(in crate::workspace) fn finish_number_column_close(
        &mut self,
        result: NumberColumnCloseResult,
        context: &egui::Context,
    ) {
        match result.decision {
            NumberColumnCloseDecision::Cancel => self.cancel_table_draft_action(),
            NumberColumnCloseDecision::Discard => {
                // Shared state already discarded only the confirmed exact draft.
                // The pending transition remains independently owned by the host.
                self.resume_table_draft_action(context);
            }
            NumberColumnCloseDecision::Save => {
                let Some(request) = result.request else {
                    self.cancel_table_draft_action();
                    return;
                };
                if request.tab != result.tab || request.draft_revision != result.draft_revision {
                    self.cancel_table_draft_action();
                    return;
                }
                self.save_number_column(request);
                self.restore_number_column_close_after_refusal();
            }
        }
    }

    /// Review refusal or an acknowledged original image leaves the field name
    /// unsaved. Keep the transition intent and offer its explicit choices again.
    /// A retained native actor or failed AFTER read never enters this path.
    pub(in crate::workspace) fn restore_number_column_close_after_refusal(&mut self) {
        if self.deferred_draft_action.is_none() || self.editing.has_save() || self.state.busy {
            return;
        }
        if let Some((tab, _, _)) = self.state.number_column_name() {
            let tab = tab.clone();
            self.state.request_number_column_close(&tab);
        }
    }

    /// Atomic schema publication retained these IDs and exact object kinds.
    /// Rebase a prior user transition onto the fresh revision without reviving
    /// any stale row handle, native result, filter handle or query capability.
    pub(super) fn number_column_transition_tab(&self, original: &ObjectTab) -> Option<ObjectTab> {
        let revision = self.state.database.as_ref()?.document_revision;
        self.state.object_tabs().into_iter().find(|tab| {
            tab.id == original.id
                && tab.target.kind == original.target.kind
                && tab.target.document_revision == revision
        })
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
