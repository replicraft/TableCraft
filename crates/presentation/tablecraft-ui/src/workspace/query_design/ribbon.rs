//! Working Design commands use the contextual Ribbon and the same typed intents.
use crate::localization::tr;
use crate::workspace::ribbon::{self, CommandSize, Icon, command, command_menu, group};
use crate::workspace::{Action, State};
use egui::{Popup, Ui, accesskit::HasPopup};

impl State {
    pub(in crate::workspace) fn query_design_context(&self) -> bool {
        self.query_design_visible()
    }

    pub(in crate::workspace) fn query_design_ribbon(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        let ready = self.query_design_ready();
        ribbon::strip(ui, |ui| {
            group(ui, &tr(ui, "ribbon-views"), |ui| {
                let view = command_menu(
                    ui,
                    Icon::ViewDesign,
                    &tr(ui, "ribbon-view"),
                    ready,
                    CommandSize::Large,
                );
                if !view.enabled() {
                    Popup::close_id(ui.ctx(), Popup::default_response_id(&view));
                }
                if view.enabled() {
                    Popup::menu(&view).show(|ui| {
                        if ui.button(tr(ui, "query-sql-view")).clicked() {
                            if let Some(action) = self.show_query_design_sql() {
                                actions.push(action);
                            }
                            ui.close();
                        }
                        ui.add_enabled(false, egui::Button::new(tr(ui, "query-design-view")));
                    });
                }
                let expanded = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&view));
                ui.ctx().accesskit_node_builder(view.id, |node| {
                    node.set_has_popup(HasPopup::Menu);
                    node.set_expanded(expanded);
                });
            });
            group(ui, &tr(ui, "qbe-results-group"), |ui| {
                let run = self.query_design_run_action();
                if command(
                    ui,
                    Icon::Run,
                    &tr(ui, "query-run"),
                    run.is_some(),
                    CommandSize::Large,
                )
                .clicked()
                    && let Some(action) = run
                {
                    actions.push(action);
                }
            });
            group(ui, &tr(ui, "qbe-setup-group"), |ui| {
                ui.horizontal_top(|ui| {
                    if command(
                        ui,
                        Icon::AddTables,
                        &tr(ui, "qbe-add-tables"),
                        ready,
                        CommandSize::Large,
                    )
                    .clicked()
                    {
                        self.show_query_design_tables();
                    }
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &self.localizer,
                            Icon::Builder,
                            "qbe-builder",
                            CommandSize::Large,
                        );
                    }
                });
            });
            if self.mock_preview {
                group(ui, &tr(ui, "qbe-show-hide-group"), |ui| {
                    for (icon, key) in [
                        (Icon::Sum, "qbe-totals"),
                        (Icon::Parameters, "qbe-parameters"),
                        (Icon::Properties, "qbe-property-sheet"),
                    ] {
                        ribbon::mock_command(ui, &self.localizer, icon, key, CommandSize::Small);
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests;
