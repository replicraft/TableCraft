//! Native menu shapes delegate solely to existing qualified working commands.
pub(in crate::workspace) mod paste;
use super::{Action, CommandSize, Icon, State, command};
use crate::workspace::ribbon::{command_menu, command_menu_inline};
use crate::{DeleteTarget, ObjectTab};
use egui::{Popup, Response, Ui, accesskit::HasPopup};

#[derive(Clone, PartialEq, Eq)]
struct DeleteMenuWitness {
    target: DeleteTarget,
    publication: u64,
    object: Option<ObjectTab>,
}

impl State {
    pub(super) fn select_menu(&mut self, ui: &mut Ui) {
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
            .on_disabled_hover_text(t.text("ribbon-unavailable"));
        });
    }

    pub(super) fn refresh_menu(
        &self,
        ui: &mut Ui,
        refresh: Option<Action>,
        actions: &mut Vec<Action>,
    ) {
        let t = self.localizer.clone();
        let response = command_menu_inline(
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
            .on_disabled_hover_text(t.text("ribbon-unavailable"));
        });
    }

    pub(super) fn delete_menu(&self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        let current = self
            .current_delete_target()
            .map(|target| DeleteMenuWitness {
                target,
                publication: self.focus_publication,
                object: self.active_object(),
            });
        let response = command_menu(
            ui,
            Icon::Delete,
            &t.text("ribbon-delete"),
            current.is_some(),
            CommandSize::Small,
        )
        .on_hover_text(t.text("deletion-checking"));
        let popup = Popup::default_response_id(&response);
        let witness_id = response.id.with("delete_review_target");
        let captured = ui.data(|data| data.get_temp::<DeleteMenuWitness>(witness_id));
        if Popup::is_id_open(ui.ctx(), popup) && captured != current {
            Popup::close_id(ui.ctx(), popup);
        }
        if !Popup::is_id_open(ui.ctx(), popup) {
            ui.data_mut(|data| data.remove::<DeleteMenuWitness>(witness_id));
        }
        show_menu(ui, &response, |ui| {
            let Some(current) = current else {
                return;
            };
            let captured = ui.data_mut(|data| {
                let captured = data
                    .get_temp::<DeleteMenuWitness>(witness_id)
                    .unwrap_or_else(|| current.clone());
                data.insert_temp(witness_id, captured.clone());
                captured
            });
            let enabled = captured == current && self.can_delete_row(&captured.target);
            if command(
                ui,
                Icon::Delete,
                &t.text("deletion-title"),
                enabled,
                CommandSize::Small,
            )
            .on_hover_text(t.text("deletion-checking"))
            .clicked()
            {
                actions.push(Action::ReviewDelete {
                    target: captured.target,
                });
                ui.close();
            }
        });
    }

    pub(super) fn zoom_menu(&mut self, ui: &mut Ui) {
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
