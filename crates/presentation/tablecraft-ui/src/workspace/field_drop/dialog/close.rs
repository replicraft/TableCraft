use super::{body_height, size};
use crate::{
    Action, ObjectKind, State,
    localization::{message, tr},
    workspace::field_drop::FieldDropCloseDecision as Decision,
};
use egui::{Context, Id, Modal};

impl State {
    pub(in crate::workspace) fn field_drop_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(close) = &self.field_drop.close else {
            return;
        };
        let ObjectKind::Table { name: table } = &close.target.tab.target.kind else {
            return;
        };
        let current = self.field_drop_close_current();
        let can_save = self.can_save_field_drop_close();
        let mut choice = None;
        let response = Modal::new(Id::new("field_drop_close")).show(context, |ui| {
            size(ui, context);
            ui.heading(tr(
                ui,
                if self.field_drop_source_stale() {
                    "field-drop-title"
                } else {
                    "field-drop-close-title"
                },
            ));
            egui::ScrollArea::vertical()
                .max_height(body_height(context))
                .show(ui, |ui| {
                    if self.field_drop_source_stale() {
                        ui.label(tr(ui, "field-drop-close-warning"));
                    } else {
                        ui.label(message(
                            ui,
                            "field-drop-close-question",
                            &[
                                ("table", table.as_str()),
                                ("column", close.target.column.as_str()),
                            ],
                        ));
                    }
                    if !current {
                        ui.label(tr(ui, "field-drop-close-stale"));
                    } else if !can_save && !self.field_drop_source_stale() {
                        ui.label(tr(ui, "field-drop-close-unavailable"));
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(can_save, egui::Button::new(tr(ui, "field-drop-close-save")))
                    .clicked()
                {
                    choice = Some(Decision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(ui, "field-drop-close-discard")),
                    )
                    .clicked()
                {
                    choice = Some(Decision::Discard);
                }
                if ui.button(tr(ui, "field-drop-close-cancel")).clicked() {
                    choice = Some(Decision::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(Decision::Cancel);
        }
        if let Some(choice) = choice
            && let Some(result) = self.choose_field_drop_close(choice)
        {
            actions.push(Action::FieldDropClose { result });
        }
    }
}
