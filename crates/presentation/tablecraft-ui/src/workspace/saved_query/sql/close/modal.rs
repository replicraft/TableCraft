//! Compact lifecycle adaptation; native Access appearance remains unqualified.
use super::{SavedQuerySqlCloseDecision, State};
use crate::localization::{message, tr};
use crate::workspace::{Action, ObjectKind};
use egui::{Context, Modal};

const MODAL_WIDTH: f32 = 420.0;

impl State {
    pub(in crate::workspace) fn saved_query_sql_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(confirmation) = &self.saved_query_sql_close else {
            return;
        };
        let ObjectKind::SavedQuery { name, .. } = &confirmation.tab.target.kind else {
            return;
        };
        let current = self.saved_query_sql_close_current();
        let can_save = self.can_save_saved_query_sql_close();
        let mut choice = None;
        let response = Modal::new(egui::Id::new("saved_query_sql_close")).show(context, |ui| {
            ui.set_max_width(MODAL_WIDTH);
            ui.heading(tr(ui, "saved-query-close-title"));
            ui.label(message(
                ui,
                "saved-query-close-question",
                &[("name", name.clone())],
            ));
            if !current {
                ui.label(tr(ui, "saved-query-close-stale"));
            } else if !can_save {
                ui.label(tr(ui, "saved-query-close-unavailable"));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(tr(ui, "saved-query-close-save")),
                    )
                    .on_disabled_hover_text(tr(ui, "saved-query-close-unavailable"))
                    .clicked()
                {
                    choice = Some(SavedQuerySqlCloseDecision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(ui, "saved-query-close-discard")),
                    )
                    .clicked()
                {
                    choice = Some(SavedQuerySqlCloseDecision::Discard);
                }
                if ui.button(tr(ui, "saved-query-close-cancel")).clicked() {
                    choice = Some(SavedQuerySqlCloseDecision::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(SavedQuerySqlCloseDecision::Cancel);
        }
        if let Some(choice) = choice
            && let Some(result) = self.choose_saved_query_sql_close(choice)
        {
            actions.push(Action::SavedQuerySqlClose { result });
        }
    }
}
