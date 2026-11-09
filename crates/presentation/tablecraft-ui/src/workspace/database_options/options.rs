//! File Options uses a shared dialog; Current Database is distinct from appearance.
use crate::{Action, State, localization::tr};
use egui::{Context, Id, Modal};

const OPTIONS_WIDTH: f32 = 820.0;
const OPTIONS_HEIGHT: f32 = 620.0;
const CATEGORY_WIDTH: f32 = 164.0;
const VIEWPORT_MARGIN: f32 = 40.0;

impl State {
    pub fn show_options_dialog(&mut self) {
        if self.database_options_pending() {
            return;
        }
        self.options_open = true;
        self.return_from_backstage();
    }

    pub fn options_dialog_open(&self) -> bool {
        self.options_open
    }

    pub fn database_options_restore_settings(&mut self) {
        self.options_open = true;
        self.options_current_database = true;
    }

    pub(in crate::workspace) fn options_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if !self.options_open {
            return;
        }
        let response = Modal::new(Id::new("application_options")).show(context, |ui| {
            ui.set_max_width(
                OPTIONS_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(160.0)),
            );
            ui.heading(tr(ui, "settings-title"));
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(CATEGORY_WIDTH.min(ui.available_width() / 3.0));
                    if ui
                        .selectable_label(
                            !self.options_current_database,
                            tr(ui, "database-options-category-general"),
                        )
                        .clicked()
                    {
                        self.options_current_database = false;
                    }
                    if ui
                        .selectable_label(
                            self.options_current_database,
                            tr(ui, "database-options-current-database"),
                        )
                        .clicked()
                    {
                        self.options_current_database = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(
                        OPTIONS_HEIGHT.min(
                            (context.viewport_rect().height() - 2.0 * VIEWPORT_MARGIN).max(120.0),
                        ),
                    )
                    .show(ui, |ui| {
                        if self.options_current_database {
                            self.database_options_pane(ui, actions);
                        } else {
                            self.appearance_options(ui);
                        }
                    });
            });
            ui.separator();
            ui.button(tr(ui, "backstage-close")).clicked()
        });
        if response.inner || response.should_close() {
            self.options_open = false;
        }
    }
}
