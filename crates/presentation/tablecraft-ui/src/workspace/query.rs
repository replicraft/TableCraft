//! Query editor admission is shared with keyboard command handling.
use super::{Action, DocumentView, State};
use crate::localization::tr;
use egui::{RichText, TextEdit, Ui};

const MAX_SQL_EDITOR_HEIGHT: f32 = 150.0;

impl State {
    pub(super) fn query_editor(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if self.view == DocumentView::Query {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!self.saved_query.active, tr(ui, "query-title"))
                    .clicked()
                {
                    self.interrupt_saved_query();
                    self.saved_query.active = false;
                }
                if let Some(target) = &self.saved_query.target
                    && ui
                        .selectable_label(self.saved_query.active, &target.name)
                        .clicked()
                {
                    self.saved_query.active = true;
                }
            });
            if self.saved_query.active {
                self.saved_query_editor(ui, actions);
                return;
            }
            let sql_label = ui.heading(tr(ui, "query-title"));
            ui.label(
                RichText::new(tr(ui, "query-help")).color(crate::theme::palette(ui.ctx()).muted),
            );
            ui.scope(|ui| {
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                egui::ScrollArea::vertical()
                    .id_salt((
                        "query_sql",
                        self.database.as_ref().map(|db| db.document_revision),
                    ))
                    .max_height(MAX_SQL_EDITOR_HEIGHT)
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        let sql = ui
                            .add(
                                TextEdit::multiline(&mut self.sql)
                                    .code_editor()
                                    .desired_rows(5)
                                    .desired_width(f32::INFINITY)
                                    .hint_text(tr(ui, "query-placeholder")),
                            )
                            .labelled_by(sql_label.id);
                        crate::theme::input_border(ui, &sql);
                    });
            });
            if ui
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
