//! File Options uses a shared dialog; Current Database is distinct from appearance.
use crate::{Action, State, localization::tr};
use egui::{Align, Context, Id, Layout, Modal};

const OPTIONS_WIDTH: f32 = 820.0;
const OPTIONS_HEIGHT: f32 = 620.0;
const CATEGORY_WIDTH: f32 = 164.0;
const VIEWPORT_MARGIN: f32 = 40.0;
const DIALOG_CHROME_HEIGHT: f32 = 100.0;

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
        let width =
            OPTIONS_WIDTH.min((context.viewport_rect().width() - 2.0 * VIEWPORT_MARGIN).max(160.0));
        let body_height = OPTIONS_HEIGHT.min(
            (context.viewport_rect().height() - 2.0 * VIEWPORT_MARGIN - DIALOG_CHROME_HEIGHT)
                .max(100.0),
        );
        let response = Modal::new(Id::new("application_options")).show(context, |ui| {
            ui.set_width(width);
            ui.heading(tr(ui, "settings-title"));
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(width, body_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    let categories = CATEGORY_WIDTH.min(width / 3.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(categories, body_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(categories);
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
                        },
                    );
                    ui.separator();
                    let content_width = ui.available_width();
                    // A ScrollArea inherits its parent's layout. Its content must
                    // own an explicit vertical layout rather than inheriting this
                    // horizontal category/body split and expanding every block right.
                    ui.allocate_ui_with_layout(
                        egui::vec2(content_width, body_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(content_width);
                            egui::ScrollArea::vertical()
                                .max_height(body_height)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_width(content_width - ui.spacing().scroll.bar_width);
                                    if self.options_current_database {
                                        self.database_options_pane(ui, actions);
                                    } else {
                                        self.appearance_options(ui);
                                    }
                                });
                        },
                    );
                },
            );
            ui.separator();
            ui.button(tr(ui, "backstage-close")).clicked()
        });
        // Options is an application-modal dialog: clicking the parent window
        // does not dismiss it. In particular, neither its opening File command
        // nor an appearance ComboBox popup may act as a backdrop dismissal.
        // Escape first belongs to an open popup, then to this topmost dialog.
        let escape = response.is_top_modal
            && !response.any_popup_open
            && context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if response.inner || escape {
            self.options_open = false;
        }
    }
}
