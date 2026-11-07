//! Unsaved table presentation, separate from native catalog and row authority.
mod close;
mod creating;
pub(in crate::workspace) mod design;
pub(super) mod field_delete;
mod gallery;
mod grid;
mod header;
mod record;
mod rename;
pub(super) mod save;
pub use save::{InitialRecord, TableDraftSaveCompletion, TableSaveRequest};

use super::objects::{ObjectKind, ObjectTab};
use super::{Action, State};
use crate::localization::tr;
use egui::Ui;
use tablecraft_builders::table::{DraftFieldType, TableDraft};

pub(super) struct Editor {
    pub(super) draft: TableDraft,
    pub(in crate::workspace) design: design::Design,
    record: record::RowDraft,
    rename: Option<rename::Rename>,
    focus_add: bool,
    save_requested: bool,
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
            design: design::Design::default(),
            record: record::RowDraft::new(),
            rename: None,
            focus_add: true,
            save_requested: false,
            error: false,
            save_name: "Table1".into(),
            save_blocked: false,
            save_message: None,
        }
    }

    pub(super) fn commit_rename(&mut self) -> bool {
        let Some(rename) = &mut self.rename else {
            return true;
        };
        if rename.rejected || self.draft.rename_field(rename.field, &rename.name).is_err() {
            self.error = true;
            rename.refocus();
            return false;
        }
        self.rename = None;
        self.error = false;
        self.focus_add = true;
        true
    }

    pub(in crate::workspace) fn view_available(&self) -> bool {
        !self.save_blocked
    }

    pub(super) fn record_revision(&self) -> u64 {
        self.record.revision()
    }

    pub(super) fn is_modified(&self) -> bool {
        self.draft.is_modified()
            || self.record.is_modified()
            || self.rename.as_ref().is_some_and(|rename| {
                rename.rejected
                    || self
                        .draft
                        .fields()
                        .iter()
                        .any(|field| field.id() == rename.field && field.name() != rename.name)
            })
    }

    pub(super) fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.draft.retained_bytes()
            + self.record.retained_bytes()
            + self
                .rename
                .as_ref()
                .map_or(0, |rename| rename.name.capacity())
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
            && !self.table_draft_field_delete_open()
            && self
                .table_draft
                .as_ref()
                .is_none_or(|editor| !editor.save_blocked);
        let can_save = self.can_save_table_draft();
        let tab = self.active_object().unwrap();
        let Some(editor) = &mut self.table_draft else {
            ui.label(tr(ui, "table-draft-unavailable"));
            return true;
        };
        let event = ui
            .add_enabled_ui(enabled, |ui| {
                ui.push_id(("initial_table_editor", tab.id), |ui| {
                    if editor.design.active {
                        design::show(ui, editor, can_save)
                    } else {
                        grid::show(ui, editor, can_save)
                    }
                })
                .inner
            })
            .inner;
        let view_requested = editor.design.requested.take();
        match event {
            Some(grid::Event::Save) => {
                self.request_table_draft_save();
            }
            Some(grid::Event::DeleteField(field)) => {
                self.request_table_draft_field_delete(field);
            }
            Some(grid::Event::AddField(kind)) => {
                self.add_table_draft_field(kind);
            }
            Some(grid::Event::ChangeType(field, kind)) => {
                self.set_table_draft_field_type(field, kind);
            }
            None => {}
        }
        if let Some(design) = view_requested {
            if design {
                self.show_table_draft_design();
            } else {
                self.show_table_draft_datasheet();
            }
        }
        true
    }

    /// Explicit field declaration; this does not infer a type from entered data.
    pub fn add_table_draft_field(&mut self, kind: DraftFieldType) -> bool {
        if self.busy
            || self.creation_is_pending()
            || self.table_draft_close.is_some()
            || self.table_draft_save.is_some()
            || self.table_draft_field_delete_open()
        {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        if editor.save_blocked || !editor.commit_rename() {
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
        if !editor.record.synchronize(&editor.draft) {
            editor.error = true;
            return false;
        }
        editor.rename = Some(rename::Rename::new(id, name.into()));
        editor.focus_add = false;
        true
    }

    pub fn table_draft_revision(&self) -> Option<u64> {
        self.table_draft
            .as_ref()
            .map(|editor| editor.draft.revision())
    }

    /// Returns true when a close confirmation now owns the transition.
    pub fn request_table_draft_discard(&mut self) -> bool {
        if self.table_draft_field_delete_open() {
            return false;
        }
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
