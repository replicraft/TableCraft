//! Shared title, database controls, status and About presentation.
use super::{Action, State};
use crate::theme::{ACCENT, INK, TITLE_FILL};
use egui::{Color32, Frame, Margin, RichText, Ui};

impl State {
    pub(super) fn chrome(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(TITLE_FILL)
                    .inner_margin(Margin::symmetric(10, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("TableCraft")
                            .color(ACCENT)
                            .strong()
                            .size(13.0),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(
                            self.database
                                .as_ref()
                                .map_or("Database workspace", |db| db.title.as_str()),
                        )
                        .color(INK),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("READ ONLY · PREVIEW").color(INK).size(11.0));
                    });
                });
            });
        if !self.backstage_visible() {
            egui::Panel::top("ribbon")
                .frame(
                    Frame::new()
                        .fill(Color32::WHITE)
                        .inner_margin(Margin::symmetric(8, 5)),
                )
                .show(ui, |ui| {
                    if ui.button("File").clicked() {
                        self.show_open();
                    }
                });
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(if self.busy { "Working…" } else { "Ready" });
                if self.busy {
                    ui.spinner();
                    if ui.button("Cancel").clicked() {
                        actions.push(Action::Cancel);
                    }
                }
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
}
