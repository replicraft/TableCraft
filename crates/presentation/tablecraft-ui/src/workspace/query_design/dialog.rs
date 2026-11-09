//! Source-bound Add Tables dialog; unsupported query sources remain unavailable.
use crate::localization::tr;
use crate::workspace::{Action, State, click, diagnostics};
use egui::{Frame, ScrollArea};

const TABLE_ROW_HEIGHT: f32 = 18.0;
const TABLE_LIST_HEIGHT: f32 = 360.0;
const MIN_TABLE_LIST_HEIGHT: f32 = 100.0;
const DIALOG_VERTICAL_RESERVE: f32 = 200.0;
const DIALOG_WIDTH: f32 = 350.0;
const DIALOG_VIEWPORT_MARGIN: f32 = 48.0;
const FOOTER_BUTTON_WIDTH: f32 = 85.0;

impl State {
    pub(in crate::workspace) fn query_design_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        if !self.query_design_dialog_open() {
            return;
        }
        let request = self.query_design_request().expect("source-bound dialog");
        let pending = self.query_design_request_id();
        let database = self.database.as_ref().expect("current source");
        let tables = || {
            database
                .tables
                .iter()
                .filter(|table| table.local && !table.system)
        };
        let mut session = self
            .query_design
            .session
            .take()
            .expect("open Add Tables dialog");
        if session
            .selected_table
            .as_ref()
            .is_none_or(|selected| !tables().any(|table| &table.name == selected))
        {
            // The Show Table dialog retains selected table identity independently of the
            // borrowed table inventory, including across frames.
            session.selected_table = tables().next().map(|table| table.name.clone());
        }
        let mut close = false;
        let mut row_clicked = false;
        egui::Modal::new(egui::Id::new(("query_design_tables", request.tab.id))).show(
            context,
            |ui| {
                ui.set_width(
                    DIALOG_WIDTH.min(context.content_rect().width() - DIALOG_VIEWPORT_MARGIN),
                );
                ui.heading(tr(ui, "qbe-add-tables"));
                ui.horizontal(|ui| {
                    let _ = ui.selectable_label(true, tr(ui, "qbe-tables"));
                    for key in ["qbe-queries", "qbe-both"] {
                        ui.add_enabled(false, egui::Button::selectable(false, tr(ui, key)))
                            .on_disabled_hover_text(tr(ui, "qbe-query-sources-unavailable"));
                    }
                });
                let list_height = TABLE_LIST_HEIGHT
                    .min(context.content_rect().height() - DIALOG_VERTICAL_RESERVE)
                    .max(MIN_TABLE_LIST_HEIGHT);
                Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ScrollArea::vertical()
                        .max_height(list_height)
                        .min_scrolled_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for table in tables() {
                                let table = &table.name;
                                let selected = session.selected_table.as_ref() == Some(table);
                                let response = ui
                                    .push_id(("qbe_table", table), |ui| {
                                        ui.add_enabled(
                                            pending.is_none(),
                                            egui::Button::selectable(selected, "")
                                                .left_text(table)
                                                .corner_radius(0.0)
                                                .min_size(egui::vec2(
                                                    ui.available_width(),
                                                    TABLE_ROW_HEIGHT,
                                                )),
                                        )
                                    })
                                    .inner;
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_label(table.as_str());
                                });
                                if response.clicked() {
                                    row_clicked = true;
                                    // The Show Table dialog retains selected table identity
                                    // independently of the borrowed table inventory, including
                                    // across frames.
                                    session.selected_table = Some(table.clone());
                                    if click::activate(ui, &response, &mut session.click_pair) {
                                        actions.push(Action::DescribeQueryDesign {
                                            // Action::DescribeQueryDesign needs its own
                                            // captured identity after this render/dispatch;
                                            // keep the original catalog, input or owner
                                            // available for the later checks in this operation.
                                            request: request.clone(),
                                            // Action::DescribeQueryDesign needs its own
                                            // captured identity after this render/dispatch;
                                            // keep the original catalog, input or owner
                                            // available for the later checks in this operation.
                                            table: table.clone(),
                                        });
                                    }
                                }
                            }
                        });
                });
                if let Some(error) = &session.error {
                    diagnostics::show(ui, ui.make_persistent_id("query_design_table_error"), error);
                }
                let labels = [tr(ui, "qbe-add"), tr(ui, "qbe-close")];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let width = labels
                    .iter()
                    .map(|label| {
                        ui.painter()
                            // Two real toolkit outputs retain the text independently
                            // (paint/layout, accessibility or hover/clipboard); the owning API
                            // consumes the String while the other output remains needed.
                            // The layout API consumes this FontId; keep the original style or
                            // loop-local descriptor available for the next measurement.
                            .layout_no_wrap(label.clone(), font.clone(), ui.visuals().text_color())
                            .size()
                            .x
                            + 2.0 * ui.spacing().button_padding.x
                    })
                    .map(|width| width.max(FOOTER_BUTTON_WIDTH))
                    .sum::<f32>()
                    + ui.spacing().item_spacing.x;
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - width).max(0.0));
                    if ui
                        .add_enabled(
                            pending.is_none() && session.selected_table.is_some(),
                            egui::Button::new(tr(ui, "qbe-add"))
                                .min_size(egui::vec2(FOOTER_BUTTON_WIDTH, 0.0)),
                        )
                        .clicked()
                    {
                        session.click_pair = None;
                        actions.push(Action::DescribeQueryDesign {
                            // Action::DescribeQueryDesign needs its own captured identity after
                            // this render/dispatch; keep the original catalog, input or owner
                            // available for the later checks in this operation.
                            request: request.clone(),
                            // Action::DescribeQueryDesign needs its own captured identity after
                            // this render/dispatch; keep the original catalog, input or owner
                            // available for the later checks in this operation.
                            table: session.selected_table.clone().expect("enabled selection"),
                        });
                    }
                    close |= ui
                        .add(
                            egui::Button::new(tr(ui, "qbe-close"))
                                .min_size(egui::vec2(FOOTER_BUTTON_WIDTH, 0.0)),
                        )
                        .clicked();
                });
                close |= ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            },
        );
        if !row_clicked && context.input(|input| input.pointer.any_released()) {
            session.click_pair = None;
        }
        if close {
            session.click_pair = None;
            session.add_tables = false;
            if let Some(request_id) = pending {
                actions.push(Action::CancelQueryDesign { request_id });
            }
        }
        self.query_design.session = Some(session);
    }
}

#[cfg(test)]
mod tests;
