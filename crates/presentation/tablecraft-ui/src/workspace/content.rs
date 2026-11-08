//! Active table/query result presentation, preserving each slot's paging state.
mod object_status;
mod tabs;

#[cfg(test)]
mod object_layout_tests;

use super::focus::{self, Pane};
use super::ribbon::Icon;
use super::{Action, DocumentView, State, diagnostics};
use crate::datasheet;
use crate::localization::{message, tr};
use egui::{Frame, Margin, RichText, Ui, WidgetInfo, WidgetType};

impl State {
    pub(super) fn content(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::CentralPanel::default()
            .frame(
                Frame::central_panel(ui.style())
                    .fill(crate::theme::palette(ui.ctx()).surface)
                    .inner_margin(Margin::ZERO),
            )
            .show(ui, |ui| {
                self.document_tabs(ui, actions);
                if let Some(object) = self.active_object() {
                    ui.push_id(("object_document", object.id), |ui| {
                        self.document_body(ui, actions);
                    });
                } else {
                    self.document_body(ui, actions);
                }
            });
    }

    fn document_body(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if let Some(pane) = self.take_pane_entry(ui.ctx()) {
            self.prepare_pane_entry(pane, ui.ctx());
        }
        if let Some(error) = &self.error {
            Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(12)
                .show(ui, |ui| {
                    diagnostics::show(ui, ui.make_persistent_id("workspace_error"), error);
                });
            ui.add_space(10.0);
        }
        if self.show_table_draft(ui) {
            return;
        }
        self.inline_insert_toolbar(ui, actions);
        if self.view == DocumentView::Table {
            self.filter_report(ui);
            self.totals_report(ui);
        }
        if self.unloaded_object_status(ui, actions) {
            return;
        }
        if self.view == DocumentView::Design {
            self.design(ui);
            return;
        }
        self.query_editor(ui, actions);
        if self.query_design_visible() {
            return;
        }
        if self.view == DocumentView::Table {
            egui::Panel::bottom(ui.make_persistent_id("record_navigation")).show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("record_toolbar")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.record_navigator(ui, actions);
                            let more = ui.menu_button(Icon::MoreRecords.image(ui, 12.0), |ui| {
                                self.filter_status(ui);
                                let before = actions.len();
                                if self.filtered_page_navigator(ui, actions) {
                                    if actions.len() != before {
                                        ui.close();
                                    }
                                    return;
                                }
                                if let Some(data) = &self.data {
                                    page_navigator(
                                        ui,
                                        data,
                                        self.busy || self.insertion_dialog_open(),
                                        actions,
                                    );
                                }
                                if actions.len() != before {
                                    ui.close();
                                }
                            });
                            let label = tr(ui, "datasheet-page-tools");
                            more.response.widget_info(|| {
                                WidgetInfo::labeled(WidgetType::Button, true, &label)
                            });
                            more.response.on_hover_text(label);
                        });
                    });
            });
        }
        let insert_target = self.current_insert_target();
        let occurrence = self.current_find_occurrence();
        let data = match self.view {
            DocumentView::Table => self.data.as_ref(),
            DocumentView::Query if self.saved_query.active => self.saved_query.data.as_ref(),
            DocumentView::Query => self.query_data.as_ref(),
            DocumentView::Design => None,
        };
        if let Some(data) = data {
            let sort_header = if self.inline_insert_active() {
                None
            } else {
                self.native_sort_header()
            };
            let totals_context = self.totals_render_context();
            let totals = totals_context
                .as_ref()
                .and_then(|(target, enabled)| self.totals.row(target, *enabled));
            let selection = if self.view == DocumentView::Table {
                &mut self.table_selection
            } else if self.saved_query.active {
                &mut self.saved_query.selection
            } else {
                &mut self.query_selection
            };
            let pane = if self.view == DocumentView::Table {
                Pane::Document
            } else {
                Pane::Results
            };
            if selection.position().is_some() {
                focus::declare(ui, pane);
            }
            let output = datasheet::show(
                ui,
                data,
                selection,
                pane,
                datasheet::Features {
                    sort_header,
                    insert_target,
                    inline_insert: if self
                        .database
                        .as_ref()
                        .map(|database| database.document_revision)
                        == data.source_revision
                    {
                        super::insertion::inline::row(&mut self.insertion, data)
                    } else {
                        None
                    },
                    totals,
                    occurrence,
                },
            );
            actions.extend(output.actions);
            self.apply_totals_selection(output.selection);
            self.apply_total_choices(output.totals_choices);
        } else if self.view == DocumentView::Query {
            Frame::new().inner_margin(10).show(ui, |ui| {
                ui.weak(tr(
                    ui,
                    if self.saved_query.active {
                        "query-saved-empty"
                    } else {
                        "query-empty"
                    },
                ));
            });
        } else if self
            .database
            .as_ref()
            .is_some_and(|database| database.tables.iter().all(|table| table.system))
        {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                ui.heading(tr(ui, "creation-title"));
                ui.add_space(12.0);
                ui.label(tr(ui, "creation-empty-database"));
                ui.label(tr(ui, "creation-table-authoring-unavailable"));
            });
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
    }

    fn prepare_pane_entry(&mut self, pane: Pane, context: &egui::Context) {
        match (pane, self.view) {
            (Pane::Document, DocumentView::Table) => self.table_selection.focus_selected(context),
            (Pane::Document, DocumentView::Design) => self.request_design_pane_entry(context),
            (Pane::Results, DocumentView::Query) => {
                if self.saved_query.active {
                    self.saved_query.selection.focus_selected(context);
                } else {
                    self.query_selection.focus_selected(context);
                }
            }
            _ => {}
        }
    }
}

fn page_navigator(ui: &mut Ui, data: &super::Data, busy: bool, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !busy && data.row_ordinals.is_none() && data.offset > 0,
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
                !busy && data.row_ordinals.is_none() && data.has_more && next_offset.is_some(),
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

#[cfg(test)]
mod pane_focus_tests;

#[cfg(test)]
mod error_tests;
