//! Revision-bound confirmation for declarations that exist only in an unsaved draft.
use crate::localization::{message, tr};
use crate::workspace::{ObjectTab, State};
use egui::{Context, Modal};
use tablecraft_builders::table::DraftFieldId;

const DIALOG_MAX_WIDTH: f32 = 420.0;
const DIALOG_VIEWPORT_MARGIN: f32 = 48.0;

pub(in crate::workspace) struct Confirmation {
    tab: ObjectTab,
    field: DraftFieldId,
    name: String,
    schema_revision: u64,
    record_revision: u64,
    property_revision: u64,
    focus_cancel: bool,
}

impl Confirmation {
    pub(in crate::workspace) fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.name.capacity()
    }
}

impl State {
    pub fn table_draft_field_delete_open(&self) -> bool {
        self.table_draft_field_delete.is_some()
    }

    pub(in crate::workspace) fn table_draft_field_delete_queued(&self) -> bool {
        self.table_draft
            .as_ref()
            .is_some_and(|editor| editor.design.delete_requested.is_some())
    }

    /// Only an existing selected Design declaration can own Delete Rows.
    pub fn can_delete_selected_table_draft_field(&self) -> bool {
        if !self.object_switch_ready()
            || self.creation_is_pending()
            || !self.table_draft_design_visible()
        {
            return false;
        }
        let Some(editor) = &self.table_draft else {
            return false;
        };
        if editor.save_blocked || editor.rename.is_some() || editor.new_field.editing() {
            return false;
        }
        let Some(field) = editor.design.selected_field(&editor.draft) else {
            return false;
        };
        !field.is_protected()
            && editor.record.removal(field.id()).is_some()
            && editor.properties.removal(field.id()).is_some()
    }

    /// Ribbon clicks resolve after this frame's raw editor input, before a
    /// confirmation captures its schema/record/property witnesses.
    pub(in crate::workspace) fn queue_delete_selected_table_draft_field(&mut self) -> bool {
        if !self.can_delete_selected_table_draft_field() {
            return false;
        }
        let editor = self.table_draft.as_mut().unwrap();
        let field = editor.design.selected_field(&editor.draft).unwrap().id();
        editor.design.delete_requested = Some(field);
        true
    }

    /// Capture an unsaved declaration and raw-record revision; never native authority.
    pub fn request_table_draft_field_delete(&mut self, field: DraftFieldId) -> bool {
        if !self.object_switch_ready() || self.creation_is_pending() {
            return false;
        }
        let Some(tab) = self.active_object() else {
            return false;
        };
        let Some(editor) = self.table_draft_editor_mut(&tab) else {
            return false;
        };
        if editor.save_blocked || editor.rename.is_some() || editor.new_field.pending() {
            return false;
        }
        let Some(declaration) = editor.draft.fields().iter().find(|declaration| {
            declaration.id() == field
                && !declaration.is_protected()
                && (editor.design.active || !declaration.is_primary())
        }) else {
            return false;
        };
        if editor.record.removal(field).is_none() || editor.properties.removal(field).is_none() {
            return false;
        }
        self.table_draft_field_delete = Some(Confirmation {
            tab,
            field,
            name: declaration.name().into(),
            schema_revision: editor.draft.revision(),
            record_revision: editor.record_revision(),
            property_revision: editor.property_revision(),
            focus_cancel: true,
        });
        true
    }

    pub fn cancel_table_draft_field_delete(&mut self) -> bool {
        self.table_draft_field_delete.take().is_some()
    }

    /// Apply only the exact still-current confirmation, without copying row input.
    pub fn confirm_table_draft_field_delete(&mut self) -> bool {
        let Some(confirmation) = self.table_draft_field_delete.take() else {
            return false;
        };
        if self.busy
            || self.creation_is_pending()
            || self.table_draft_save.is_some()
            || self.table_draft_close.is_some()
            || self.active_object().as_ref() != Some(&confirmation.tab)
            || self
                .database
                .as_ref()
                .map(|database| database.document_revision)
                != Some(confirmation.tab.target.document_revision)
        {
            return false;
        }
        let Some(editor) = self.table_draft_editor_mut(&confirmation.tab) else {
            return false;
        };
        if editor.save_blocked
            || editor.rename.is_some()
            || editor.new_field.pending()
            || editor.draft.revision() != confirmation.schema_revision
            || editor.record_revision() != confirmation.record_revision
            || editor.property_revision() != confirmation.property_revision
            || !editor.draft.fields().iter().any(|field| {
                field.id() == confirmation.field
                    && !field.is_protected()
                    && (editor.design.active || !field.is_primary())
                    && field.name() == confirmation.name
            })
        {
            return false;
        }
        let Some((position, record_revision)) = editor.record.removal(confirmation.field) else {
            return false;
        };
        let Some((property_position, property_revision)) =
            editor.properties.removal(confirmation.field)
        else {
            return false;
        };
        // Builder removal is allocation-free and validates its own revision
        // before mutation. The preflighted Vec removal below cannot fail.
        if editor.draft.remove_field(confirmation.field).is_err() {
            return false;
        }
        editor.record.remove_prepared(position, record_revision);
        editor.descriptions.remove(confirmation.field);
        editor
            .properties
            .remove_prepared(property_position, property_revision);
        editor.focus_add = true;
        editor.error = false;
        editor.save_requested = false;
        true
    }

    pub(in crate::workspace) fn table_draft_field_delete_dialog(&mut self, context: &Context) {
        let Some(confirmation) = &mut self.table_draft_field_delete else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let response = Modal::new(egui::Id::new("table_draft_field_delete")).show(context, |ui| {
            ui.set_max_width(
                (context.content_rect().width() - DIALOG_VIEWPORT_MARGIN)
                    .clamp(1.0, DIALOG_MAX_WIDTH),
            );
            ui.heading(tr(ui, "table-draft-delete-field-title"));
            ui.label(message(
                ui,
                "table-draft-delete-field-question",
                &[("field", confirmation.name.as_str())],
            ));
            ui.label(tr(ui, "table-draft-delete-field-warning"));
            ui.horizontal_wrapped(|ui| {
                let cancelled = ui.button(tr(ui, "table-draft-cancel"));
                if confirmation.focus_cancel {
                    cancelled.request_focus();
                    confirmation.focus_cancel = false;
                }
                cancel = cancelled.clicked();
                confirm = ui
                    .button(tr(ui, "table-draft-delete-field-confirm"))
                    .clicked();
            });
        });
        cancel |= response.should_close();
        if cancel {
            self.cancel_table_draft_field_delete();
        } else if confirm {
            self.confirm_table_draft_field_delete();
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod design_tests;
