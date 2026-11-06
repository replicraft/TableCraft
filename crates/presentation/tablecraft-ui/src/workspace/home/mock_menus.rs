//! Mock-only native menu shapes delegate solely to existing working commands.
use super::{Action, CommandSize, Icon, State, command};
use crate::workspace::ribbon::command_menu;
use egui::{Popup, Response, Ui, accesskit::HasPopup};

impl State {
    pub(super) fn mock_select_menu(&mut self, ui: &mut Ui) {
        let t = self.localizer.clone();
        let enabled = self.can_select_record();
        let response = command_menu(
            ui,
            Icon::Cursor,
            &t.text("ribbon-select"),
            enabled,
            CommandSize::Small,
        );
        show_menu(ui, &response, |ui| {
            if command(
                ui,
                Icon::Cursor,
                &t.text("ribbon-select-record"),
                enabled,
                CommandSize::Small,
            )
            .clicked()
            {
                self.select_current_record(ui.ctx());
                ui.close();
            }
            command(
                ui,
                Icon::Selection,
                &t.text("ribbon-select-all"),
                false,
                CommandSize::Small,
            )
            .on_disabled_hover_text(t.text("ribbon-preview-only"));
        });
    }

    pub(super) fn mock_refresh_menu(
        &self,
        ui: &mut Ui,
        refresh: Option<Action>,
        actions: &mut Vec<Action>,
    ) {
        let t = self.localizer.clone();
        let response = command_menu(
            ui,
            Icon::Refresh,
            &t.text("ribbon-refresh-all"),
            refresh.is_some(),
            CommandSize::Large,
        );
        show_menu(ui, &response, |ui| {
            if command(
                ui,
                Icon::Refresh,
                &t.text("ribbon-refresh-table"),
                refresh.is_some(),
                CommandSize::Small,
            )
            .on_hover_text(t.text("ribbon-refresh-table-help"))
            .clicked()
            {
                if let Some(action) = refresh {
                    actions.push(action);
                }
                ui.close();
            }
            command(
                ui,
                Icon::Refresh,
                &t.text("ribbon-refresh-all"),
                false,
                CommandSize::Small,
            )
            .on_disabled_hover_text(t.text("ribbon-preview-only"));
        });
    }

    pub(super) fn mock_zoom_menu(&mut self, ui: &mut Ui) {
        let t = self.localizer.clone();
        let enabled = self.can_zoom_cell();
        let response = command_menu(
            ui,
            Icon::Zoom,
            &t.text("zoom-title"),
            enabled,
            CommandSize::Large,
        );
        show_menu(ui, &response, |ui| {
            if command(
                ui,
                Icon::Zoom,
                &t.text("zoom-title"),
                enabled,
                CommandSize::Small,
            )
            .on_hover_text(t.text("zoom-read-only"))
            .clicked()
            {
                self.open_zoom_cell();
                ui.close();
            }
        });
    }
}

fn show_menu(ui: &mut Ui, response: &Response, contents: impl FnOnce(&mut Ui)) {
    let popup = Popup::default_response_id(response);
    if response.enabled() {
        Popup::menu(response).show(contents);
    } else {
        Popup::close_id(ui.ctx(), popup);
    }
    let expanded = Popup::is_id_open(ui.ctx(), popup);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(expanded);
    });
}

#[cfg(test)]
mod tests;
