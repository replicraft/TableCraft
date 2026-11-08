//! Unsaved table presentation, separate from native catalog and row authority.
mod close;
mod creating;
pub(in crate::workspace) mod design;
pub(super) mod field_delete;
mod gallery;
mod grid;
mod header;
mod manual;
mod record;
mod rename;
pub(super) mod save;
pub use save::{InitialRecord, TableDraftSaveCompletion, TableSaveRequest};

use super::objects::{ObjectKind, ObjectTab};
use super::{Action, State};
use crate::localization::tr;
use design::properties::editor::Editors;
use egui::Ui;
use tablecraft_builders::table::{DraftFieldType, TableDraft};

pub(super) struct Editor {
    pub(super) draft: TableDraft,
    pub(in crate::workspace) design: design::Design,
    record: record::RowDraft,
    properties: Editors,
    descriptions: design::description::Editors,
    new_field: design::new_field::NewField,
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
    pub(super) fn initial(save_name: String) -> Self {
        Self::new(save_name, TableDraft::initial(), false)
    }

    pub(super) fn design(save_name: String) -> Self {
        Self::new(save_name, TableDraft::design(), true)
    }

    fn new(save_name: String, draft: TableDraft, design: bool) -> Self {
        Self {
            draft,
            design: design::Design::new(design),
            record: record::RowDraft::new(),
            properties: Editors::default(),
            descriptions: design::description::Editors::default(),
            new_field: design::new_field::NewField::new(design),
            rename: None,
            focus_add: true,
            save_requested: false,
            error: false,
            save_name,
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

    pub(super) fn commit_pending(&mut self) -> bool {
        self.commit_rename() && self.commit_new_field()
    }

    pub(super) fn commit_new_field(&mut self) -> bool {
        if !self.new_field.pending() {
            return true;
        }
        if self.new_field.rejected || self.new_field.name.is_empty() {
            self.new_field.refuse();
            self.error = true;
            return false;
        }
        let Some(record_revision) = self.record.prepare_append(&self.draft) else {
            self.new_field.refuse();
            self.error = true;
            return false;
        };
        let kind = self.new_field.kind;
        let Ok(id) = self.draft.add_named_field(kind, &self.new_field.name) else {
            self.new_field.refuse();
            self.error = true;
            return false;
        };
        self.record.append_prepared(id, kind, record_revision);
        self.design.select(id);
        self.new_field.committed();
        self.error = false;
        true
    }

    /// Reserve the row entry before publishing an automatically named field.
    /// Initial typed callers may not have rendered/synchronized the ID yet.
    fn add_field(&mut self, kind: DraftFieldType) -> bool {
        if self.save_blocked || !self.commit_pending() {
            return false;
        }
        if !self.record.synchronize(&self.draft) {
            self.error = true;
            return false;
        }
        let Some(record_revision) = self.record.prepare_append(&self.draft) else {
            self.error = true;
            return false;
        };
        let Ok(id) = self.draft.add_field(kind) else {
            self.error = true;
            return false;
        };
        self.record.append_prepared(id, kind, record_revision);
        let field = self.draft.fields().last().unwrap();
        self.rename = Some(rename::Rename::new(id, field.name().into()));
        self.focus_add = false;
        self.error = false;
        true
    }

    pub(in crate::workspace) fn view_available(&self) -> bool {
        !self.save_blocked
    }

    pub(super) fn property_revision(&self) -> u64 {
        self.properties.revision()
    }

    pub(super) fn record_revision(&self) -> u64 {
        self.record.revision()
    }

    pub(super) fn is_modified(&self) -> bool {
        self.draft.is_modified()
            || self.new_field.pending()
            || self.record.is_modified()
            || self.properties.rejected()
            || self.descriptions.rejected()
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
            + self.properties.retained_bytes()
            + self.descriptions.retained_bytes()
            + self.new_field.retained_bytes()
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
        let can_delete = self.can_delete_selected_table_draft_field();
        let tab = self.active_object().unwrap();
        let Some(editor) = &mut self.table_draft else {
            ui.label(tr(ui, "table-draft-unavailable"));
            return true;
        };
        let event = ui
            .add_enabled_ui(enabled, |ui| {
                ui.push_id(("initial_table_editor", tab.id), |ui| {
                    if editor.design.active {
                        design::show(ui, editor, can_save, can_delete)
                    } else {
                        grid::show(ui, editor, can_save)
                    }
                })
                .inner
            })
            .inner;
        let view_requested = editor.design.requested.take();
        let delete_requested = editor.design.delete_requested.take();
        match event {
            Some(grid::Event::Save) => {
                self.request_table_draft_save();
            }
            Some(grid::Event::DeleteField(field)) => {
                self.request_table_draft_field_delete(field);
            }
            Some(grid::Event::ChangeType(field, kind)) => {
                self.set_table_draft_field_type(field, kind);
            }
            None => {}
        }
        if let Some(field) = delete_requested {
            self.request_table_draft_field_delete(field);
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
        editor.add_field(kind)
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
