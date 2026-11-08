//! Saved SQL uses the existing SQL editor style; layout is a visual adaptation.
use crate::localization::tr;
use crate::workspace::{
    State, diagnostics,
    focus::{self, Pane},
    query::parameters::copy_text,
};
use egui::{RichText, TextEdit, Ui};
use std::borrow::Cow;
use tablecraft_builders::query::MAX_DESIGN_TEXT_BYTES;

use super::SavedQuerySqlError;

const MAX_SQL_EDITOR_HEIGHT: f32 = 150.0;
const MIN_SQL_EDITOR_HEIGHT: f32 = 24.0;

impl State {
    pub(super) fn saved_query_sql_editor(&mut self, ui: &mut Ui) {
        let label = ui.label(tr(ui, "query-title"));
        if let Some(error) = self.saved_query_sql_error() {
            let message = match error {
                SavedQuerySqlError::SourceChanged => {
                    Cow::Owned(self.localizer.text("saved-query-sql-source-changed"))
                }
                SavedQuerySqlError::MemoryLimit => {
                    Cow::Owned(self.localizer.text("objects-memory-limit"))
                }
                SavedQuerySqlError::Host(message) => Cow::Borrowed(message.as_str()),
            };
            let target = self
                .selected_saved_target()
                .map(|target| (target.document_revision, target.object_id));
            let revision = self.saved_query_sql().map(|(_, _, revision, _)| revision);
            let id = ui.make_persistent_id(("saved_query_sql_error", target, revision));
            diagnostics::show(ui, id, &message);
        }
        let Some((_, proposed, _, _)) = self.saved_query_sql() else {
            ui.weak(tr(ui, "ribbon-unavailable"));
            return;
        };
        let bytes = proposed.len();
        if !self.admit_object_bytes(Some(bytes)) {
            return;
        }
        let Some(mut text) = self
            .saved_query_sql()
            .and_then(|(_, proposed, _, _)| copy_text(proposed))
        else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return;
        };
        let Some((document_revision, object_id)) = self
            .selected_saved_target()
            .map(|target| (target.document_revision, target.object_id))
        else {
            return;
        };
        let editable = self.can_edit_saved_query_sql();
        let editor_id = ui.make_persistent_id(("saved_query_sql", document_revision, object_id));
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical()
                .id_salt(("saved_query_sql_scroll", document_revision, object_id))
                .min_scrolled_height(MIN_SQL_EDITOR_HEIGHT)
                .max_height(MAX_SQL_EDITOR_HEIGHT)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    if editable {
                        crate::text_input::reject_oversized_events(
                            ui,
                            editor_id,
                            &text,
                            MAX_DESIGN_TEXT_BYTES,
                        );
                    }
                    let response = ui
                        .add_enabled(
                            editable,
                            TextEdit::multiline(&mut text)
                                .id(editor_id)
                                .code_editor()
                                .desired_rows(5)
                                .desired_width(f32::INFINITY)
                                .char_limit(crate::text_input::MAX_EDITOR_SCALARS),
                        )
                        .labelled_by(label.id)
                        .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
                    if response.changed() && editable {
                        self.set_saved_query_sql_text(text);
                    }
                    crate::theme::input_border(ui, &response);
                    focus::register(ui, Pane::Document, &response, true);
                });
        });
        if self.saved_query_sql_dirty() {
            ui.label(
                RichText::new(tr(ui, "saved-query-sql-unsaved"))
                    .color(crate::theme::palette(ui.ctx()).muted),
            );
        }
        if self.saved_query_sql_pending() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(ui, "saved-query-working"));
            });
        }
    }
}

#[cfg(test)]
mod tests;
