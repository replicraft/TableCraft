//! Compact close adaptation; no retained Access saved-field close capture exists.
use super::NumberColumnCloseDecision as Decision;
use crate::{
    localization::{message, tr},
    workspace::{Action, State},
};
use egui::{Context, Modal};

const MODAL_WIDTH: f32 = 420.0;

impl State {
    pub(in crate::workspace) fn number_column_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(confirmation) = &self.number_column.close else {
            return;
        };
        let Some(table) = super::super::table_name(&confirmation.tab) else {
            return;
        };
        let column = self
            .number_column
            .draft()
            .map_or("", |(_, name, _, _)| name);
        let current = self.number_column_close_current();
        let can_save = self.can_save_number_column_close();
        let mut choice = None;
        let response = Modal::new(egui::Id::new("number_column_close")).show(context, |ui| {
            ui.set_max_width(MODAL_WIDTH);
            ui.heading(tr(ui, "datasheet-number-close-title"));
            ui.label(message(
                ui,
                "datasheet-number-close-question",
                &[("table", table.as_str().into()), ("column", column.into())],
            ));
            if !current {
                ui.label(tr(ui, "datasheet-number-close-stale"));
            } else if !can_save {
                ui.label(tr(ui, "datasheet-number-close-unavailable"));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(tr(ui, "datasheet-number-close-save")),
                    )
                    .on_disabled_hover_text(tr(ui, "datasheet-number-close-unavailable"))
                    .clicked()
                {
                    choice = Some(Decision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(ui, "datasheet-number-close-discard")),
                    )
                    .clicked()
                {
                    choice = Some(Decision::Discard);
                }
                if ui.button(tr(ui, "datasheet-number-close-cancel")).clicked() {
                    choice = Some(Decision::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(Decision::Cancel);
        }
        if let Some(choice) = choice
            && let Some(result) = self.choose_number_column_close(choice)
        {
            actions.push(Action::NumberColumnClose { result });
        }
    }
}
