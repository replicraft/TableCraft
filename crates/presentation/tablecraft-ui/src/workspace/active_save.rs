//! One active-object Save route; capture saved SQL only after document widgets.
use super::{Action, ObjectKind, State};

#[derive(Default)]
pub(super) struct PendingSave {
    saved_query_tab: Option<u64>,
    creation_tab: Option<u64>,
    saved_design_tab: Option<u64>,
    field_addition_tab: Option<u64>,
    field_rename_tab: Option<u64>,
    field_drop_tab: Option<u64>,
}

impl PendingSave {
    pub(in crate::workspace) fn is_pending(&self) -> bool {
        self.saved_design_tab.is_some()
            || self.creation_tab.is_some()
            || self.saved_query_tab.is_some()
            || self.field_addition_tab.is_some()
            || self.field_rename_tab.is_some()
            || self.field_drop_tab.is_some()
    }
}

impl State {
    /// Presentation availability; native authority is issued by the host review.
    pub fn can_save_active_object(&self) -> bool {
        self.can_begin_query_creation()
            || self.can_save_saved_query_design()
            || self.can_save_table_draft()
            || self.can_review_saved_query_sql()
            || self.can_review_field_addition()
            || self.can_review_field_rename()
            || self.can_review_field_drop()
    }

    pub(in crate::workspace) fn queue_active_save(&mut self) -> bool {
        if self.can_save_saved_query_design()
            && let Some(tab) = self.active_object()
        {
            self.active_save.saved_design_tab = Some(tab.id);
            return true;
        }
        if self.can_begin_query_creation()
            && let Some(tab) = self.active_object()
        {
            self.active_save.creation_tab = Some(tab.id);
            return true;
        }
        if self.can_review_field_drop()
            && let Some(tab) = self.active_object()
        {
            self.active_save.field_drop_tab = Some(tab.id);
            return true;
        }
        if self.can_edit_field_rename()
            && let Some(tab) = self.active_object()
        {
            self.active_save.field_rename_tab = Some(tab.id);
            return true;
        }
        if let Some(tab) = self.active_object()
            && self.can_edit_field_addition_name(&tab)
        {
            self.active_save.field_addition_tab = Some(tab.id);
            return true;
        }
        if self.can_save_table_draft() {
            return self.queue_table_draft_save();
        }
        if !self.can_edit_saved_query_sql() || self.saved_query_sql_pending() {
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
        if self.can_begin_query_creation_save_as()
            && context.input_mut(|input| {
                input.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::S,
                )
            })
        {
            self.active_save.creation_tab = self.active_object().map(|tab| tab.id);
            return;
        }
        let saved_sql_editor_ready =
            self.can_edit_saved_query_sql() && !self.saved_query_sql_pending();
        let field_header_ready = self
            .active_object()
            .is_some_and(|tab| self.can_edit_field_addition_name(&tab));
        if (self.can_save_active_object()
            || saved_sql_editor_ready
            || field_header_ready
            || self.can_edit_field_rename())
            && context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.queue_active_save();
        }
    }

    pub(in crate::workspace) fn finish_active_save(&mut self, actions: &mut Vec<Action>) {
        if let Some(id) = self.active_save.saved_design_tab.take() {
            if self.active_object().is_some_and(|tab| tab.id == id)
                && self.can_save_saved_query_design()
                && let Some(action) = self.query_design_save_action_for(true)
            {
                actions.push(action);
            }
            return;
        }
        if let Some(id) = self.active_save.creation_tab.take() {
            if self.active_object().is_some_and(|tab| tab.id == id)
                && self.can_begin_query_creation_save_as()
            {
                if self.query_design_visible() {
                    if let Some(action) = self.query_design_save_action() {
                        actions.push(action);
                    }
                } else {
                    self.begin_query_creation_name();
                }
            }
            return;
        }
        if let Some(expected_tab) = self.active_save.field_drop_tab.take() {
            if self
                .active_object()
                .is_some_and(|tab| tab.id == expected_tab)
                && self.can_review_field_drop()
            {
                if let Some(request) = self.field_drop_review_request() {
                    actions.push(Action::ReviewFieldDrop { request });
                } else {
                    self.error = Some(self.localizer.text("objects-memory-limit"));
                }
            }
            return;
        }
        if let Some(expected_tab) = self.active_save.field_rename_tab.take() {
            if self
                .active_object()
                .is_some_and(|tab| tab.id == expected_tab)
                && self.can_review_field_rename()
            {
                if let Some(request) = self.field_rename_review_request() {
                    actions.push(Action::ReviewFieldRename { request });
                } else {
                    self.error = Some(self.localizer.text("objects-memory-limit"));
                }
            }
            return;
        }
        if let Some(expected) = self.active_save.field_addition_tab.take() {
            if self.active_object().is_some_and(|tab| tab.id == expected)
                && let Some(request) = self.field_addition_review_request()
            {
                actions.push(Action::ReviewFieldAddition { request });
            }
            return;
        }
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

impl State {
    fn can_save_saved_query_design(&self) -> bool {
        self.saved_query.active
            && self.query_design_visible()
            && self.can_begin_query_creation_save_as()
            && self.query_design_request().is_some()
    }
}

impl State {
    pub(in crate::workspace) fn queue_query_creation_save_as(&mut self) -> bool {
        if !self.can_begin_query_creation_save_as() {
            return false;
        }
        self.active_save.creation_tab = self.active_object().map(|tab| tab.id);
        self.active_save.creation_tab.is_some()
    }
}
