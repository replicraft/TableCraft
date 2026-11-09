//! Retained bounded Description cells; the builder owns accepted metadata.
use crate::localization::{message, tr};
use egui::Ui;
use tablecraft_builders::table::{
    DraftField, DraftFieldId, MAX_DRAFT_DESCRIPTION_BYTES, MAX_DRAFT_DESCRIPTION_UNITS, TableDraft,
};

#[derive(Default)]
pub(in crate::workspace::table_draft) struct Editors {
    entries: Vec<Entry>,
    allocation_rejected: bool,
}

pub(super) struct EditResponse {
    pub(super) changed: bool,
    pub(super) response: egui::Response,
}

struct Entry {
    field: DraftFieldId,
    text: String,
    rejected: bool,
    pending: bool,
}

impl Editors {
    pub(super) fn synchronize(&mut self, draft: &TableDraft) -> bool {
        self.entries
            .retain(|entry| draft.fields().iter().any(|field| field.id() == entry.field));
        for field in draft.fields() {
            if self.entries.iter().any(|entry| entry.field == field.id()) {
                continue;
            }
            let text = field.description().unwrap_or_default();
            let mut owned = String::new();
            if owned.try_reserve_exact(text.len()).is_err()
                || self.entries.try_reserve_exact(1).is_err()
            {
                self.allocation_rejected = true;
                return false;
            }
            owned.push_str(text);
            self.entries.push(Entry {
                field: field.id(),
                text: owned,
                rejected: false,
                pending: false,
            });
        }
        self.allocation_rejected = false;
        true
    }

    pub(super) fn show_inert(&self, ui: &mut Ui, field: &DraftField) -> egui::Response {
        let mut text = self
            .entries
            .iter()
            .find(|entry| entry.field == field.id())
            .map_or("", |entry| entry.text.as_str());
        let output = egui::TextEdit::multiline(&mut text)
            .id(ui.make_persistent_id(("draft_field_description", field.id())))
            .interactive(false)
            .desired_rows(1)
            .desired_width(ui.available_width())
            .margin(egui::Margin::symmetric(2, 1))
            .show(ui);
        let response = output.response.response;
        let label = message(
            ui,
            "table-draft-design-description",
            &[("field", field.name())],
        );
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label));
        response.interact(egui::Sense::CLICK)
    }

    pub(super) fn show(
        &mut self,
        ui: &mut Ui,
        field: &DraftField,
        focus: bool,
    ) -> Option<EditResponse> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.field == field.id())?;
        let id = ui.make_persistent_id(("draft_field_description", field.id()));
        let original = entry.text.clone();
        let rejected = crate::text_input::reject_events_with_limits(
            ui,
            id,
            &original,
            MAX_DRAFT_DESCRIPTION_BYTES,
            MAX_DRAFT_DESCRIPTION_UNITS,
        );
        let label = message(
            ui,
            "table-draft-design-description",
            &[("field", field.name())],
        );
        // One visible row retains multiline input exactly rather than replacing
        // pasted whitespace. The containing Design cell owns clipping/height.
        let output = egui::TextEdit::multiline(&mut entry.text)
            .id(id)
            .desired_rows(1)
            .desired_width(ui.available_width())
            .margin(egui::Margin::symmetric(2, 1))
            .show(ui);
        let mut response = output.response.response;
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(label);
        });
        if focus {
            response.request_focus();
        }
        crate::theme::input_border(ui, &response);
        let changed = rejected || response.changed();
        if rejected
            || entry.text.len() > MAX_DRAFT_DESCRIPTION_BYTES
            || entry.text.encode_utf16().count() > MAX_DRAFT_DESCRIPTION_UNITS
        {
            entry.text = original;
            entry.rejected = true;
            entry.pending = false;
        } else if response.changed() {
            entry.rejected = false;
            entry.pending = true;
        }
        if entry.rejected {
            response = response.on_hover_text(tr(ui, "table-draft-description-invalid"));
        }
        Some(EditResponse { changed, response })
    }

    pub(super) fn reject(&mut self, field: DraftFieldId) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.rejected = true;
            entry.pending = false;
        }
    }

    pub(in crate::workspace::table_draft) fn remove(&mut self, field: DraftFieldId) {
        self.entries.retain(|entry| entry.field != field);
    }

    pub(super) fn commit(&mut self, draft: &mut TableDraft) {
        for entry in &mut self.entries {
            if !std::mem::take(&mut entry.pending) {
                continue;
            }
            let value = (!entry.text.is_empty()).then_some(entry.text.as_str());
            if draft.set_field_description(entry.field, value).is_err() {
                entry.rejected = true;
            }
        }
    }

    pub(in crate::workspace::table_draft) fn rejected(&self) -> bool {
        self.allocation_rejected || self.entries.iter().any(|entry| entry.rejected)
    }

    pub(in crate::workspace::table_draft) fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.entries.capacity() * size_of::<Entry>()
            + self
                .entries
                .iter()
                .map(|entry| entry.text.capacity())
                .sum::<usize>()
    }
}
