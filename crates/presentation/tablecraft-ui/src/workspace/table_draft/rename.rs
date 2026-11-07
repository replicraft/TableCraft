//! Inline field-header naming; the builder remains the declaration authority.
use crate::localization::tr;
use egui::{
    Ui,
    text::{CCursor, CCursorRange},
};
use tablecraft_builders::table::{DraftFieldId, MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

const HORIZONTAL_PADDING: f32 = 2.0;
const VERTICAL_PADDING: f32 = 1.0;

pub(super) struct Rename {
    pub(super) field: DraftFieldId,
    pub(super) name: String,
    focus: bool,
    select_all: bool,
    pub(super) rejected: bool,
}

pub(super) enum Outcome {
    Commit,
    Cancel,
}

impl Rename {
    pub(super) fn new(field: DraftFieldId, name: String) -> Self {
        Self {
            field,
            name,
            focus: true,
            select_all: true,
            rejected: false,
        }
    }

    pub(super) fn refocus(&mut self) {
        self.focus = true;
    }

    pub(super) fn show(&mut self, ui: &mut Ui) -> Option<Outcome> {
        let input_id = ui.make_persistent_id(("draft_field_name", self.field));
        let focused = ui.memory(|memory| memory.has_focus(input_id));
        let (cancel, commit) = ui.input_mut(|input| {
            if !focused {
                return (false, false);
            }
            let cancel = input.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            let commit = input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            (cancel, commit)
        });
        let original = self.name.clone();
        let rejected = crate::text_input::reject_events_with_limits(
            ui,
            input_id,
            &original,
            MAX_DRAFT_NAME_BYTES,
            MAX_DRAFT_NAME_UNITS,
        );
        let mut output = egui::TextEdit::singleline(&mut self.name)
            .id(input_id)
            .desired_width(ui.available_width())
            .event_filter(egui::EventFilter {
                tab: true,
                escape: true,
                horizontal_arrows: true,
                vertical_arrows: false,
            })
            .margin(egui::vec2(HORIZONTAL_PADDING, VERTICAL_PADDING))
            .show(ui);
        let response = &output.response;
        let label = tr(ui, "table-draft-field-name");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), &label)
        });
        if rejected
            || self.name.len() > MAX_DRAFT_NAME_BYTES
            || self.name.encode_utf16().count() > MAX_DRAFT_NAME_UNITS
        {
            self.name = original;
            self.rejected = true;
        } else if response.changed() {
            self.rejected = false;
        }
        if self.focus {
            response.request_focus();
            response.scroll_to_me(None);
            self.focus = false;
        }
        if self.select_all {
            output.state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0),
                CCursor::new(self.name.chars().count()),
            )));
            output.state.store(ui.ctx(), input_id);
            self.select_all = false;
        }
        if cancel {
            Some(Outcome::Cancel)
        } else if commit || response.lost_focus() {
            Some(Outcome::Commit)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
