//! Unsaved table presentation, separate from native catalog and row authority.
mod close;
mod creating;
mod grid;
pub(super) mod save;
pub use save::{TableDraftSaveCompletion, TableSaveRequest};

use super::objects::{ObjectKind, ObjectTab};
use super::{Action, State};
use crate::localization::tr;
use egui::Ui;
use tablecraft_builders::table::{DraftFieldId, DraftFieldType, TableDraft};

pub(super) struct Editor {
    pub(super) draft: TableDraft,
    rename: Option<(DraftFieldId, String)>,
    focus_add: bool,
    error: bool,
    save_name: String,
    save_blocked: bool,
    save_message: Option<String>,
}

#[derive(Clone)]
pub(super) enum CloseIntent {
    Tab(ObjectTab),
    All,
}

impl Editor {
    pub(super) fn initial() -> Self {
        Self {
            draft: TableDraft::initial(),
            rename: None,
            focus_add: true,
            error: false,
            save_name: "Table1".into(),
            save_blocked: false,
            save_message: None,
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.draft.retained_bytes()
            + self.rename.as_ref().map_or(0, |(_, text)| text.capacity())
            + self.save_name.capacity()
            + self.save_message.as_ref().map_or(0, String::capacity)
    }
}

impl State {
    pub(super) fn show_table_draft(&mut self, ui: &mut Ui) -> bool {
        if !self
            .active_object()
            .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
        {
            return false;
        }
        let enabled = !self.busy
            && !self.creation_is_pending()
            && self.table_draft_close.is_none()
            && self.table_draft_save.is_none()
            && self
                .table_draft
                .as_ref()
                .is_none_or(|editor| !editor.save_blocked);
        let can_save = self.can_save_table_draft();
        let Some(editor) = &mut self.table_draft else {
            ui.label(tr(ui, "table-draft-unavailable"));
            return true;
        };
        let save = ui
            .add_enabled_ui(enabled, |ui| grid::show(ui, editor, can_save))
            .inner;
        if save {
            self.request_table_draft_save();
        }
        true
    }

    /// Explicit field declaration; this does not infer a type from entered data.
    pub fn add_table_draft_field(&mut self, kind: DraftFieldType) -> bool {
        if self.busy
            || self.creation_is_pending()
            || self.table_draft_close.is_some()
            || self.table_draft_save.is_some()
        {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        if editor.save_blocked {
            return false;
        }
        let Ok(id) = editor.draft.add_field(kind) else {
            return false;
        };
        let name = editor
            .draft
            .fields()
            .iter()
            .find(|field| field.id() == id)
            .unwrap()
            .name();
        editor.rename = Some((id, name.into()));
        true
    }

    pub fn table_draft_revision(&self) -> Option<u64> {
        self.table_draft
            .as_ref()
            .map(|editor| editor.draft.revision())
    }

    /// Returns true when a close confirmation now owns the transition.
    pub fn request_table_draft_discard(&mut self) -> bool {
        if let Some(intent) = &self.table_draft_close {
            return matches!(intent, CloseIntent::All);
        }
        if self.has_modified_table_drafts() {
            self.table_draft_close = Some(CloseIntent::All);
            true
        } else {
            self.discard_all_table_drafts();
            false
        }
    }
}

#[cfg(test)]
mod tests;
