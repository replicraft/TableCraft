//! Object opening pairs only live rows from an unchanged Navigation Pane.
use super::group;
use crate::workspace::click;
use egui::{Key, Modifiers, Popup, Response, Ui};

#[derive(Default)]
pub(in crate::workspace) struct Clicks {
    scope: Option<Scope>,
    frame: Option<u64>,
    pair: Option<click::Pair>,
    clicked: bool,
}

#[derive(PartialEq, Eq)]
pub(super) struct Scope {
    pub revision: Option<u64>,
    pub filter: String,
    pub show_system: bool,
    pub expansion: group::Expansion,
    pub publication: u64,
    pub ready: bool,
}

impl Clicks {
    pub(super) fn begin(&mut self, ui: &Ui, scope: Scope) {
        let frame = ui.ctx().cumulative_frame_nr();
        let continuous = self.frame.and_then(|previous| previous.checked_add(1)) == Some(frame);
        if !continuous || self.scope.as_ref() != Some(&scope) || !scope.ready || !ui.is_enabled() {
            self.pair = None;
        }
        self.scope = Some(scope);
        self.frame = Some(frame);
        self.clicked = false;
    }

    pub(super) fn opened(&mut self, ui: &mut Ui, response: &Response, enter_pressed: bool) -> bool {
        if !response.enabled() || !self.scope.as_ref().is_some_and(|scope| scope.ready) {
            return false;
        }
        if Popup::is_any_open(ui.ctx()) {
            self.pair = None;
            return false;
        }
        if response.has_focus() && enter_pressed {
            ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
            self.pair = None;
            return true;
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            self.clicked = true;
            return click::activate(ui, response, &mut self.pair);
        }
        false
    }

    pub(super) fn finish(&mut self, ui: &Ui) {
        if !self.clicked && ui.input(|input| input.pointer.any_released()) {
            self.pair = None;
        }
    }
}
