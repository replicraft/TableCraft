//! Query editor admission is shared with keyboard command handling.
pub(super) mod provenance;

use super::{Action, DocumentView, State, ribbon};
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use egui::{Frame, Margin, RichText, TextEdit, Ui};
use provenance::ResultOrigin;

const MAX_SQL_EDITOR_HEIGHT: f32 = 150.0;
const MIN_SQL_EDITOR_HEIGHT: f32 = 24.0;
const MIN_RESULTS_HEIGHT: f32 = 60.0;

impl State {
    fn query_result_status(&self, ui: &mut Ui) {
        if self.query_data.is_some() {
            let key = match &self.query_result {
                ResultOrigin::Unknown => "query-results-unknown",
                _ if self.query_results_previous() => "query-results-previous",
                _ => "query-results-current",
            };
            let status = RichText::new(tr(ui, key)).color(crate::theme::palette(ui.ctx()).muted);
            ui.label(status);
        }
    }

    pub(super) fn query_editor(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if self.view != DocumentView::Query {
            return;
        }
        Frame::new()
            .inner_margin(Margin::same(10))
            .show(ui, |ui| self.query_editor_contents(ui, actions));
    }

    fn query_editor_contents(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if self.view == DocumentView::Query {
            if self.saved_query.active {
                self.saved_query_editor(ui, actions);
                return;
            }
            self.query_design_report(ui);
            if self.query_design_results_visible() {
                let designed = matches!(self.query_result, ResultOrigin::Design(_));
                if designed && self.query_data.is_some() && self.query_results_previous() {
                    ui.weak(tr(ui, "qbe-results-previous"));
                } else if !designed {
                    self.query_result_status(ui);
                }
                return;
            }
            if self.query_design_visible() {
                self.query_design_editor(ui);
                return;
            }
            let sql_label = ui.heading(tr(ui, "query-title"));
            ui.label(
                RichText::new(tr(ui, "query-help")).color(crate::theme::palette(ui.ctx()).muted),
            );
            self.query_result_status(ui);
            let editor_height = if self.error.is_some() {
                let reserved = ui.spacing().interact_size.y
                    + ui.spacing().item_spacing.y * 3.0
                    + MIN_RESULTS_HEIGHT;
                (ui.available_height() - reserved)
                    .clamp(MIN_SQL_EDITOR_HEIGHT, MAX_SQL_EDITOR_HEIGHT)
            } else {
                MAX_SQL_EDITOR_HEIGHT
            };
            ui.scope(|ui| {
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                egui::ScrollArea::vertical()
                    .id_salt((
                        "query_sql",
                        self.database.as_ref().map(|db| db.document_revision),
                    ))
                    // Egui otherwise enforces a 64-point minimum over this dynamic maximum.
                    .min_scrolled_height(MIN_SQL_EDITOR_HEIGHT)
                    .max_height(editor_height)
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        let editable = self.query_sql_revision().is_some();
                        let sql = ui
                            .add_enabled(
                                editable,
                                TextEdit::multiline(&mut self.sql)
                                    .code_editor()
                                    .desired_rows(5)
                                    .desired_width(f32::INFINITY)
                                    .hint_text(tr(ui, "query-placeholder")),
                            )
                            .labelled_by(sql_label.id);
                        if sql.changed() {
                            self.record_query_sql_edit();
                        }
                        crate::theme::input_border(ui, &sql);
                        focus::register(ui, Pane::Document, &sql, true);
                    });
            });
            let run_in_ribbon =
                self.ribbon_tab == ribbon::Tab::QueryDesign && self.query_design_context();
            if !run_in_ribbon
                && ui
                    .add_enabled(self.query_ready(), egui::Button::new(tr(ui, "query-run")))
                    .clicked()
            {
                actions.push(Action::Query);
            }
            ui.separator();
        }
    }
}

#[cfg(test)]
mod tests;
