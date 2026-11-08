//! One active-object Save route; capture saved SQL only after document widgets.
use super::{Action, ObjectKind, State};

#[derive(Default)]
pub(super) struct PendingSave {
    saved_query_tab: Option<u64>,
}

impl State {
    /// Presentation availability; native authority is issued by the host review.
    pub fn can_save_active_object(&self) -> bool {
        self.can_save_table_draft() || self.can_review_saved_query_sql()
    }

    pub(in crate::workspace) fn queue_active_save(&mut self) -> bool {
        if self.can_save_table_draft() {
            return self.queue_table_draft_save();
        }
        if !self.can_review_saved_query_sql() {
            return false;
        }
        let Some(tab) = self.active_object() else {
            return false;
        };
        if !matches!(tab.target.kind, ObjectKind::SavedQuery { .. }) {
            return false;
        }
        // Tab IDs are never reused. Retain no SQL or target-name copy before
        // this frame's widgets have applied their exact text and draft revision.
        self.active_save.saved_query_tab = Some(tab.id);
        true
    }

    pub(in crate::workspace) fn consume_active_save_shortcut(&mut self, context: &egui::Context) {
        if self.can_save_active_object()
            && context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.queue_active_save();
        }
    }

    pub(in crate::workspace) fn finish_active_save(&mut self, actions: &mut Vec<Action>) {
        let Some(expected_tab) = self.active_save.saved_query_tab.take() else {
            return;
        };
        if !self
            .active_object()
            .is_some_and(|tab| tab.id == expected_tab)
            || !self.can_review_saved_query_sql()
        {
            return;
        }
        if let Some(request) = self.saved_query_sql_review_request() {
            actions.push(Action::ReviewSavedQuerySql { request });
        } else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
    }
}

#[cfg(test)]
pub(in crate::workspace) mod tests;
