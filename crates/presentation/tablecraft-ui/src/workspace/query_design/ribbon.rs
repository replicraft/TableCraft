//! Working Design commands use the contextual Ribbon and the same typed intents.
use crate::localization::tr;
use crate::workspace::ribbon::{self, CommandSize, Icon, command, group};
use crate::workspace::{Action, State};
use egui::{Popup, Ui};

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
                let view = command(
                    ui,
                    Icon::ViewDesign,
                    &tr(ui, "ribbon-view"),
                    ready,
                    CommandSize::Large,
                );
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
            });
        });
    }
}
