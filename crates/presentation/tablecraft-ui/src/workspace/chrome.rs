//! Shared title, database controls, status and About presentation.
use super::{Action, State};
use crate::theme::ACCENT;
use egui::{Color32, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

impl State {
    pub(super) fn chrome(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(ACCENT)
                    .inner_margin(Margin::symmetric(18, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("TABLECRAFT")
                            .color(Color32::WHITE)
                            .strong()
                            .size(17.0),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(
                            self.database
                                .as_ref()
                                .map_or("Database workspace", |db| db.title.as_str()),
                        )
                        .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("About").color(Color32::WHITE))
                                    .fill(ACCENT),
                            )
                            .clicked()
                        {
                            self.about = true;
                        }
                        ui.label(
                            RichText::new("READ ONLY · PREVIEW")
                                .color(Color32::WHITE)
                                .size(11.0),
                        );
                    });
                });
            });
        egui::Panel::top("ribbon").show(ui, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.query_view, false, "Table");
                ui.selectable_value(&mut self.query_view, true, "Query");
            });
            ui.separator();
            ui.horizontal(|ui| {
                let path_label = ui.label("Database path");
                ui.add_sized(
                    [(ui.available_width().min(530.0) - 190.0).max(150.0), 28.0],
                    TextEdit::singleline(&mut self.path)
                        .id(path_id)
                        .hint_text("Path to .accdb or .mdb"),
                )
                .labelled_by(path_label.id);
                let password_label = ui.label("Password");
                ui.add_sized(
                    [130.0, 28.0],
                    TextEdit::singleline(&mut self.password)
                        .password(true)
                        .hint_text("Password"),
                )
                .labelled_by(password_label.id);
                if ui
                    .add_enabled(
                        !self.path.trim().is_empty(),
                        egui::Button::new("Open database"),
                    )
                    .clicked()
                {
                    actions.push(Action::Open);
                }
                if self.busy {
                    ui.spinner();
                    if ui.button("Cancel").clicked() {
                        actions.push(Action::Cancel);
                    }
                }
            });
            ui.small("Open a stable copy or close other database writers before opening.");
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(if self.busy { "Working…" } else { "Ready" });
                ui.separator();
                ui.small(
                    self.database
                        .as_ref()
                        .map_or("Local Access databases", |db| db.format.as_str()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small("Snapshot browsing · original file unchanged");
                });
            });
        });
    }

    pub(super) fn about_window(&mut self, ui: &mut Ui) {
        egui::Window::new("About TableCraft")
            .open(&mut self.about)
            .show(ui.ctx(), |ui| {
                ui.heading("TableCraft");
                ui.label("A cross-platform Access workspace. Desktop preview.");
                ui.label("Selawik regular · © 2015 Microsoft Corporation");
                ui.hyperlink_to(
                    "Font source: Microsoft Selawik 1.01",
                    "https://github.com/microsoft/Selawik/releases/tag/1.01",
                );
                egui::CollapsingHeader::new("Selawik license").show(ui, |ui| {
                    ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        ui.label(include_str!(
                            "../../../../../assets/ui/fonts/OFL-Selawik.txt"
                        ));
                    });
                });
            });
    }
}
