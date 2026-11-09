//! Compact close adaptation; no retained Access saved-field close capture exists.
use super::FieldAdditionCloseDecision as Decision;
use crate::{
    localization::{message, tr},
    workspace::{Action, State},
};
use egui::{Context, Modal};

const MODAL_WIDTH: f32 = 420.0;

impl State {
    pub(in crate::workspace) fn field_addition_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(confirmation) = &self.field_addition.close else {
            return;
        };
        let Some(table) = super::super::table_name(&confirmation.tab) else {
            return;
        };
        let column = self
            .field_addition
            .draft()
            .map_or("", |(_, name, _, _)| name);
        let current = self.field_addition_close_current();
        let can_save = self.can_save_field_addition_close();
        let mut choice = None;
        let response = Modal::new(egui::Id::new("field_addition_close")).show(context, |ui| {
            ui.set_max_width(MODAL_WIDTH);
            let saved = self.field_addition_source_stale();
            ui.heading(tr(
                ui,
                if saved {
                    "datasheet-field-abandon-title"
                } else {
                    "datasheet-field-close-title"
                },
            ));
            if saved {
                ui.label(tr(ui, "datasheet-field-abandon-question"));
            } else {
                ui.label(message(
                    ui,
                    "datasheet-field-close-question",
                    &[("table", table.as_str().into()), ("column", column.into())],
                ));
            }
            if !current {
                ui.label(tr(ui, "datasheet-field-close-stale"));
            } else if !can_save {
                ui.label(tr(ui, "datasheet-field-close-unavailable"));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(tr(ui, "datasheet-field-close-save")),
                    )
                    .on_disabled_hover_text(tr(ui, "datasheet-field-close-unavailable"))
                    .clicked()
                {
                    choice = Some(Decision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(
                            ui,
                            if saved {
                                "datasheet-field-abandon-discard"
                            } else {
                                "datasheet-field-close-discard"
                            },
                        )),
                    )
                    .clicked()
                {
                    choice = Some(Decision::Discard);
                }
                if ui.button(tr(ui, "datasheet-field-close-cancel")).clicked() {
                    choice = Some(Decision::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(Decision::Cancel);
        }
        if let Some(choice) = choice
            && let Some(result) = self.choose_field_addition_close(choice)
        {
            actions.push(Action::FieldAdditionClose { result });
        }
    }
}
