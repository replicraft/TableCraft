//! Active table/query result presentation, preserving each slot's paging state.
use super::{Action, DocumentView, State};
use crate::{
    datasheet,
    theme::{ACCENT, MUTED},
};
use egui::{Color32, Frame, RichText, Ui};

impl State {
    pub(super) fn content(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).fill(Color32::WHITE))
            .show(ui, |ui| {
                let mut selected_view = None;
                ui.horizontal(|ui| {
                    let table_title = self
                        .data
                        .as_ref()
                        .map_or("Table", |data| data.table.as_str());
                    for (view, title) in [
                        (DocumentView::Table, table_title),
                        (DocumentView::Query, "Query"),
                        (DocumentView::Design, "Table Design"),
                    ] {
                        if view == DocumentView::Design && self.schema.is_none() {
                            continue;
                        }
                        let response = ui.selectable_label(self.view == view, title);
                        if self.view == view {
                            ui.painter().line_segment(
                                [response.rect.left_bottom(), response.rect.right_bottom()],
                                egui::Stroke::new(2.0, ACCENT),
                            );
                        }
                        if response.clicked() {
                            selected_view = Some(view);
                            ui.ctx().request_repaint();
                        }
                    }
                });
                if let Some(view) = selected_view {
                    self.select_view(view);
                }
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
                if self.view == DocumentView::Design {
                    self.design(ui);
                    return;
                }
                self.query_editor(ui, actions);
                if self.view == DocumentView::Table {
                    egui::Panel::bottom("record_navigation").show(ui, |ui| {
                        self.record_navigator(ui, actions);
                    });
                }
                let data = match self.view {
                    DocumentView::Table => self.data.as_ref(),
                    DocumentView::Query => self.query_data.as_ref(),
                    DocumentView::Design => None,
                };
                if let Some(data) = data {
                    ui.horizontal(|ui| {
                        ui.heading(&data.table);
                        ui.label(
                            RichText::new(format!("{} columns", data.columns.len())).color(MUTED),
                        );
                    });
                    if self.view == DocumentView::Table {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.busy && data.offset > 0,
                                    egui::Button::new("‹ Previous page"),
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
                                    egui::Button::new("Next page ›"),
                                )
                                .clicked()
                            {
                                actions.push(Action::Page {
                                    table: data.table.clone(),
                                    offset: next_offset
                                        .expect("enabled only for a valid next page"),
                                });
                            }
                        });
                    }
                    ui.separator();
                    if data.rows.is_empty() {
                        ui.weak("No rows.");
                    } else {
                        let selection = if self.view == DocumentView::Table {
                            &mut self.table_selection
                        } else {
                            &mut self.query_selection
                        };
                        actions.extend(datasheet::show(ui, data, selection));
                    }
                } else if self.view == DocumentView::Query {
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
