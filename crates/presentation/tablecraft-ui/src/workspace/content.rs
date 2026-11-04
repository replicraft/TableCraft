//! Active table/query result presentation, preserving each slot's paging state.
use super::{Action, State};
use crate::{
    datasheet,
    theme::{ACCENT, MUTED},
};
use egui::{Color32, Frame, RichText, Ui};

impl State {
    pub(super) fn content(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                let table_title = self
                    .data
                    .as_ref()
                    .map_or("Table", |data| data.table.as_str());
                for (query, title) in [(false, table_title), (true, "Query")] {
                    let response = ui.selectable_label(self.query_view == query, title);
                    if self.query_view == query {
                        ui.painter().line_segment(
                            [response.rect.left_bottom(), response.rect.right_bottom()],
                            egui::Stroke::new(2.0, ACCENT),
                        );
                    }
                    if response.clicked() {
                        self.query_view = query;
                    }
                }
            });
            ui.separator();
            if let Some(error) = &self.error {
                Frame::new()
                    .fill(Color32::from_rgb(255, 237, 237))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.colored_label(ACCENT, error);
                    });
                ui.add_space(10.0);
            }
            self.query_editor(ui, actions);
            if let Some(data) = self.visible_data() {
                ui.horizontal(|ui| {
                    ui.heading(&data.table);
                    ui.label(RichText::new(format!("{} columns", data.columns.len())).color(MUTED));
                });
                if !self.query_view {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.busy && data.offset > 0,
                                egui::Button::new("‹ Previous"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Page {
                                table: data.table.clone(),
                                offset: data.offset.saturating_sub(100),
                            });
                        }
                        ui.label(format!(
                            "Rows {}–{}",
                            if data.rows.is_empty() {
                                0
                            } else {
                                u64::from(data.offset) + 1
                            },
                            u64::from(data.offset) + data.rows.len() as u64
                        ));
                        let next_offset = data.offset.checked_add(100);
                        if ui
                            .add_enabled(
                                !self.busy && data.has_more && next_offset.is_some(),
                                egui::Button::new("Next ›"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Page {
                                table: data.table.clone(),
                                offset: next_offset.expect("enabled only for a valid next page"),
                            });
                        }
                    });
                }
                ui.separator();
                if data.rows.is_empty() {
                    ui.weak("No rows.");
                } else {
                    datasheet::show(ui, data);
                }
            } else if self.query_view {
                ui.weak("Run a query to see its results.");
            } else {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Your data. A familiar workspace.").size(29.0));
                    ui.add_space(12.0);
                    ui.label(
                        "Open an Access database, then choose a table to browse its contents.",
                    );
                    ui.add_space(8.0);
                    ui.weak("MDB and ACCDB · tables and supported SELECT queries");
                });
            }
        });
    }
}
