//! Query editor admission is shared with keyboard command handling.
use super::{Action, DocumentView, State};
use crate::theme::MUTED;
use egui::{RichText, TextEdit, Ui};

impl State {
    pub(super) fn query_editor(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if self.view == DocumentView::Query {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!self.saved_query.active, "SQL query")
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
            let sql_label = ui.heading("SQL query");
            ui.label(
                RichText::new("Run a supported SELECT against the opened snapshot.").color(MUTED),
            );
            ui.add(
                TextEdit::multiline(&mut self.sql)
                    .code_editor()
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text("SELECT TOP 100 * FROM [Customers]"),
            )
            .labelled_by(sql_label.id);
            if ui
                .add_enabled(self.query_ready(), egui::Button::new("Run query"))
                .clicked()
            {
                actions.push(Action::Query);
            }
            ui.separator();
        }
    }
}
