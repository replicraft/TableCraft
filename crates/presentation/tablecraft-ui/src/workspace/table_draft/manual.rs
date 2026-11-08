//! Manual Design key commands never change an existing stored table.
use super::super::{ObjectKind, State};

impl State {
    pub fn table_draft_primary_key_selected(&self) -> bool {
        self.table_draft_design_visible()
            && self.table_draft.as_ref().is_some_and(|editor| {
                editor
                    .design
                    .selected_field(&editor.draft)
                    .is_some_and(|field| field.is_primary())
            })
    }

    pub fn can_toggle_table_draft_primary_key(&self) -> bool {
        if !self.object_switch_ready()
            || self.creation_is_pending()
            || !self.table_draft_design_visible()
            || !self
                .active_object()
                .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
        {
            return false;
        }
        let Some(editor) = &self.table_draft else {
            return false;
        };
        if editor.save_blocked
            || editor.rename.is_some()
            || !editor.draft.is_design()
            || editor
                .draft
                .fields()
                .iter()
                .any(|field| field.is_protected() && field.is_primary())
        {
            return false;
        }
        let selected = editor.design.selected_field(&editor.draft);
        selected.is_some_and(|field| {
            !field.is_protected() && (field.is_primary() || field.supports_manual_primary_key())
        })
    }

    pub fn toggle_table_draft_primary_key(&mut self) -> bool {
        if !self.can_toggle_table_draft_primary_key() {
            return false;
        }
        let editor = self.table_draft.as_mut().unwrap();
        let Some(field) = editor.design.selected_field(&editor.draft) else {
            return false;
        };
        let selected = (!field.is_primary()).then_some(field.id());
        editor.draft.set_primary_key(selected).is_ok()
    }
}

#[cfg(test)]
mod tests;
