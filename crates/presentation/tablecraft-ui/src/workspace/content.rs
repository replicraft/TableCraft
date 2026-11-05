//! Active table/query result presentation, preserving each slot's paging state.
use super::{Action, DocumentView, State};
use crate::datasheet;
use crate::localization::{message, tr};
use egui::{Frame, Margin, RichText, Ui};

impl State {
    pub(super) fn content(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::CentralPanel::default()
            .frame(
                Frame::central_panel(ui.style())
                    .fill(crate::theme::palette(ui.ctx()).surface)
                    .inner_margin(Margin::ZERO),
            )
            .show(ui, |ui| {
                self.document_tabs(ui);
                if let Some(error) = &self.error {
                    Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.colored_label(crate::theme::palette(ui.ctx()).accent, error);
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
                        egui::ScrollArea::horizontal()
                            .id_salt("record_toolbar")
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    self.record_navigator(ui, actions);
                                    if let Some(data) = &self.data {
                                        ui.separator();
                                        page_navigator(ui, data, self.busy, actions);
                                    }
                                });
                            });
                    });
                }
                let data = match self.view {
                    DocumentView::Table => self.data.as_ref(),
                    DocumentView::Query if self.saved_query.active => {
                        self.saved_query.data.as_ref()
                    }
                    DocumentView::Query => self.query_data.as_ref(),
                    DocumentView::Design => None,
                };
                if let Some(data) = data {
                    if data.rows.is_empty() {
                        ui.weak(tr(ui, "datasheet-no-rows"));
                    }

                    let selection = if self.view == DocumentView::Table {
                        &mut self.table_selection
                    } else if self.saved_query.active {
                        &mut self.saved_query.selection
                    } else {
                        &mut self.query_selection
                    };
                    actions.extend(datasheet::show(ui, data, selection));
                } else if self.view == DocumentView::Query {
                    ui.weak(tr(
                        ui,
                        if self.saved_query.active {
                            "query-saved-empty"
                        } else {
                            "query-empty"
                        },
                    ));
                } else {
                    ui.add_space(80.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(tr(ui, "shell-welcome")).size(29.0));
                        ui.add_space(12.0);
                        ui.label(tr(ui, "shell-welcome-help"));
                        ui.add_space(8.0);
                        ui.weak(tr(ui, "shell-supported-files"));
                    });
                }
            });
    }

    fn document_tabs(&mut self, ui: &mut Ui) {
        let mut selected_view = None;
        Frame::new()
            .inner_margin(Margin {
                left: 4,
                right: 4,
                top: 1,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal(|ui| {
                    let table_title = self
                        .data
                        .as_ref()
                        .map(|data| data.table.clone())
                        .unwrap_or_else(|| tr(ui, "shell-table"));
                    for (view, title) in [
                        (DocumentView::Table, table_title),
                        (DocumentView::Query, tr(ui, "shell-query")),
                        (DocumentView::Design, tr(ui, "shell-table-design")),
                    ] {
                        if view == DocumentView::Design && self.schema.is_none() {
                            continue;
                        }
                        let response = ui.selectable_label(self.view == view, title);
                        ui.ctx().accesskit_node_builder(response.id, |node| {
                            node.set_role(egui::accesskit::Role::Tab);
                            node.clear_toggled();
                            node.set_selected(self.view == view);
                        });
                        if self.view == view {
                            ui.painter().line_segment(
                                [response.rect.left_bottom(), response.rect.right_bottom()],
                                egui::Stroke::new(2.0, crate::theme::palette(ui.ctx()).accent),
                            );
                        }
                        if response.clicked() {
                            selected_view = Some(view);
                            ui.ctx().request_repaint();
                        }
                    }
                });
            });
        if let Some(view) = selected_view {
            self.select_view(view);
        }
    }
}

fn page_navigator(ui: &mut Ui, data: &super::Data, busy: bool, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !busy && data.offset > 0,
                egui::Button::new(tr(ui, "datasheet-previous-page")),
            )
            .clicked()
        {
            actions.push(Action::Page {
                table: data.table.clone(),
                offset: data.offset.saturating_sub(100),
            });
        }
        ui.label(message(
            ui,
            "datasheet-row-range",
            &[
                (
                    "first",
                    if data.rows.is_empty() {
                        0
                    } else {
                        u64::from(data.offset) + 1
                    }
                    .to_string(),
                ),
                (
                    "last",
                    (u64::from(data.offset) + data.rows.len() as u64).to_string(),
                ),
            ],
        ));
        let next_offset = data.offset.checked_add(100);
        if ui
            .add_enabled(
                !busy && data.has_more && next_offset.is_some(),
                egui::Button::new(tr(ui, "datasheet-next-page")),
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
