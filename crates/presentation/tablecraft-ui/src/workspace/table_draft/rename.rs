//! Inline field-header naming; the builder remains the declaration authority.
use crate::localization::tr;
use egui::{
    Ui,
    text::{CCursor, CCursorRange},
};
use std::borrow::Cow;
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
        let mut edited_name = Cow::Borrowed(self.name.as_str());
        let rejected = crate::text_input::reject_events_with_limits(
            ui,
            input_id,
            edited_name.as_ref(),
            MAX_DRAFT_NAME_BYTES,
            MAX_DRAFT_NAME_UNITS,
        );
        let mut output = egui::TextEdit::singleline(&mut edited_name)
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
            || edited_name.len() > MAX_DRAFT_NAME_BYTES
            || edited_name.encode_utf16().count() > MAX_DRAFT_NAME_UNITS
        {
            self.rejected = true;
        } else if response.changed() {
            if let Cow::Owned(name) = edited_name {
                self.name = name;
            }
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
