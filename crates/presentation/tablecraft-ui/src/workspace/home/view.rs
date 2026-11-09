//! The existing table views share the Access-style large View dropdown.
use super::{Action, CommandSize, DocumentView, Icon, State, command};
use crate::workspace::objects::ObjectKind;
use crate::workspace::ribbon::command_menu;
use egui::{Popup, accesskit::HasPopup};

impl State {
    pub(super) fn view_menu(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.query_design_request().is_some() {
            self.query_design_view_menu(ui, actions);
            return;
        }
        let t = self.localizer.clone();
        let active = self.active_object();
        let table = active.as_ref().and_then(|tab| match &tab.target.kind {
            ObjectKind::Table { name } => Some(name.as_str()),
            _ => None,
        });
        let draft = active
            .as_ref()
            .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }));
        let datasheet_available = active
            .as_ref()
            .is_some_and(|tab| self.can_view_object_datasheet(tab));
        let enabled = active
            .as_ref()
            .is_some_and(|tab| (table.is_some() || draft) && self.can_activate_object(tab));
        let icon = if self.view == DocumentView::Design {
            Icon::ViewDatasheet
        } else {
            Icon::ViewDesign
        };
        let response = command_menu(
            ui,
            icon,
            &t.text("ribbon-view"),
            enabled,
            CommandSize::Large,
        );

        if !response.enabled() {
            Popup::close_id(ui.ctx(), Popup::default_response_id(&response));
        }
        if response.enabled() {
            Popup::menu(&response).show(|ui| {
                if command(
                    ui,
                    Icon::ViewDatasheet,
                    &t.text("ribbon-datasheet"),
                    datasheet_available,
                    CommandSize::Small,
                )
                .clicked()
                {
                    if draft {
                        self.queue_table_draft_view(false);
                    } else if let Some(tab) = active.as_ref() {
                        actions.push(Action::ViewObjectDatasheet { tab: tab.clone() });
                    }
                    ui.close();
                }
                if command(
                    ui,
                    Icon::ViewDesign,
                    &t.text("ribbon-table-design"),
                    table.is_some() || draft,
                    CommandSize::Small,
                )
                .clicked()
                {
                    if draft {
                        self.queue_table_draft_view(true);
                    } else {
                        actions.push(Action::ViewSchema {
                            table: table.expect("enabled for an existing table").to_owned(),
                        });
                    }
                    ui.close();
                }
            });
        }
        let expanded = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(expanded);
        });
    }
}
