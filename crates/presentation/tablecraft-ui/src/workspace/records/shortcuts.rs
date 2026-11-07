//! Access New-record keyboard intent uses the existing reviewed insertion service.
use crate::keyboard;
use crate::workspace::{Action, State};
use egui::{Popup, Ui};

impl State {
    pub(in crate::workspace) fn new_record_shortcut(&self, ui: &Ui, actions: &mut Vec<Action>) {
        if !ui.is_enabled()
            || !self.object_switch_ready()
            || Popup::is_any_open(ui.ctx())
            || !self.native_record_has_focus(ui)
        {
            return;
        }
        let Some(target) = self.current_insert_target() else {
            return;
        };
        // Context's built-in keyboard zoom runs at end_pass. Remove only this
        // table-owned chord before it can also scale the application.
        if keyboard::consume_new_record_shortcut(ui.ctx()) && actions.is_empty() {
            actions.push(Action::BeginInsert { target });
        }
    }
}

#[cfg(test)]
mod tests;
