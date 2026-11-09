//! One bounded, uncommitted Design row. It never owns a draft field identifier.
use super::{properties::editor::Editors, type_key, types};
use crate::localization::tr;
use egui::{Key, Modifiers, Ui, WidgetInfo, WidgetType};
use std::borrow::Cow;
use tablecraft_builders::table::{DraftFieldType, MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Name,
    Type,
}

pub(in crate::workspace::table_draft) struct NewField {
    pub(in crate::workspace::table_draft) name: String,
    pub(in crate::workspace::table_draft) kind: DraftFieldType,
    pub(in crate::workspace::table_draft) rejected: bool,
    focus: Option<Focus>,
    type_chosen: bool,
    active: bool,
    type_id: Option<egui::Id>,
}

impl Default for NewField {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: DraftFieldType::ShortText,
            rejected: false,
            focus: None,
            type_chosen: false,
            active: false,
            type_id: None,
        }
    }
}

impl NewField {
    pub(in crate::workspace::table_draft) fn new(focused: bool) -> Self {
        Self {
            focus: focused.then_some(Focus::Name),
            active: focused,
            ..Self::default()
        }
    }

    pub(in crate::workspace::table_draft) fn editing(&self) -> bool {
        self.active || self.pending()
    }

    pub(in crate::workspace::table_draft) fn deactivate(&mut self) {
        self.active = false;
        self.focus = None;
    }

    pub(in crate::workspace::table_draft) fn pending(&self) -> bool {
        !self.name.is_empty() || self.type_chosen || self.rejected
    }

    pub(in crate::workspace::table_draft) fn has_name(&self) -> bool {
        !self.name.is_empty()
    }

    pub(in crate::workspace::table_draft) fn retained_bytes(&self) -> usize {
        self.name.capacity()
    }

    pub(in crate::workspace::table_draft) fn refuse(&mut self) {
        self.rejected = true;
        self.focus = Some(Focus::Name);
    }

    pub(in crate::workspace::table_draft) fn committed(&mut self) {
        self.name.clear();
        self.kind = DraftFieldType::ShortText;
        self.rejected = false;
        self.type_chosen = false;
        self.active = false;
        self.focus = Some(Focus::Name);
    }

    fn cancel(&mut self, editors: &mut Editors) {
        if !self.pending() || !editors.record_raw_edit() {
            return;
        }
        self.committed();
    }

    pub(super) fn show_name(&mut self, ui: &mut Ui, editors: &mut Editors) -> bool {
        let id = ui.make_persistent_id("new_design_field_name");
        let focused = ui.memory(|memory| memory.has_focus(id));
        let declaration_keys = focused && !egui::Popup::is_any_open(ui.ctx());
        let escape_owner = owns_escape(ui, id);
        let (cancel, commit, next) = ui.input_mut(|input| {
            (
                escape_owner && input.consume_key(Modifiers::NONE, Key::Escape),
                declaration_keys && input.consume_key(Modifiers::NONE, Key::Enter),
                declaration_keys && input.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        let mut name = Cow::Borrowed(self.name.as_str());
        let rejected = crate::text_input::reject_events_with_limits(
            ui,
            id,
            name.as_ref(),
            MAX_DRAFT_NAME_BYTES,
            MAX_DRAFT_NAME_UNITS,
        );
        let response = egui::TextEdit::singleline(&mut name)
            .id(id)
            .desired_width(ui.available_width())
            .margin(egui::Margin::symmetric(2, 1))
            .event_filter(egui::EventFilter {
                tab: true,
                escape: true,
                horizontal_arrows: true,
                vertical_arrows: false,
            })
            .show(ui)
            .response;
        let label = tr(ui, "table-draft-design-new-name");
        response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, ui.is_enabled(), &label));
        if rejected || response.changed() {
            if !editors.record_raw_edit()
                || rejected
                || name.len() > MAX_DRAFT_NAME_BYTES
                || name.encode_utf16().count() > MAX_DRAFT_NAME_UNITS
            {
                self.rejected = true;
            } else {
                if let Cow::Owned(name) = name {
                    self.name = name;
                }
                self.rejected = false;
            }
        }
        self.active = response.has_focus();
        if self.focus == Some(Focus::Name) {
            response.request_focus();
            self.focus = None;
        }
        if next {
            self.focus = Some(Focus::Type);
        }
        if cancel {
            self.cancel(editors);
            false
        } else {
            commit && self.pending()
        }
    }

    pub(super) fn show_type(&mut self, ui: &mut Ui, editors: &mut Editors) -> bool {
        let popup_was_open = self
            .type_id
            .is_some_and(|id| egui::ComboBox::is_open(ui.ctx(), id));
        let popup_escape = popup_was_open && ui.input(|input| input.key_pressed(Key::Escape));
        let focused = self
            .type_id
            .is_some_and(|id| ui.memory(|memory| memory.has_focus(id)));
        let declaration_keys = focused && !egui::Popup::is_any_open(ui.ctx());
        let escape_owner = self.type_id.is_some_and(|id| owns_escape(ui, id));
        let (cancel, commit) = ui.input_mut(|input| {
            (
                escape_owner && input.consume_key(Modifiers::NONE, Key::Escape),
                declaration_keys
                    && (input.consume_key(Modifiers::NONE, Key::Enter)
                        || input.consume_key(Modifiers::NONE, Key::Tab)),
            )
        });
        let selected = type_key(self.kind);
        let mut selected_now = false;
        let output = egui::ComboBox::from_id_salt("new_design_field_type")
            .selected_text(if self.pending() {
                tr(ui, selected)
            } else {
                String::new()
            })
            .width(ui.available_width())
            .height(types::menu_height(ui))
            .show_ui(ui, |ui| {
                if let Some(kind) = types::show(ui, self.kind, |_| true) {
                    selected_now = true;
                    if (kind != self.kind || !self.type_chosen) && editors.record_raw_edit() {
                        self.kind = kind;
                        self.type_chosen = true;
                    }
                }
            });
        self.type_id = Some(output.response.id);
        let label = tr(ui, "table-draft-design-new-type");
        output
            .response
            .widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), &label));
        self.active |= output.response.has_focus() || output.response.clicked();
        let popup_closed_with_escape =
            popup_escape && !egui::ComboBox::is_open(ui.ctx(), output.response.id);
        if selected_now || popup_closed_with_escape || self.focus == Some(Focus::Type) {
            output.response.request_focus();
            self.focus = None;
        }
        if cancel {
            self.cancel(editors);
            false
        } else {
            commit && self.pending()
        }
    }
}

// egui may clear keyboard focus for Escape before this UI pass, particularly
// immediately after focus moves from the type selector to the name cell. Use
// only this exact widget's preceding-frame focus, with no intervening popup or
// other focused widget. This never restores a stale editor or source identity.
fn owns_escape(ui: &Ui, id: egui::Id) -> bool {
    let popup_open = egui::Popup::is_any_open(ui.ctx());
    !popup_open
        && ui.memory(|memory| {
            memory.has_focus(id) || (memory.focused().is_none() && memory.had_focus_last_frame(id))
        })
}
