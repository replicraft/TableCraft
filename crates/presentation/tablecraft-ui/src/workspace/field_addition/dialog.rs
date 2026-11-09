//! Field-addition owner status; it never routes through a cell-edit dialog.
use super::{FieldAdditionSaveCommand as Command, status::StatusPhase};
use crate::workspace::{diagnostics, save_display};
use crate::{Action, State, localization::tr};
use egui::{Context, Id, Modal};

const MAX_WIDTH: f32 = 520.0;
const MAX_REPORT_HEIGHT: f32 = 240.0;
const VIEWPORT_MARGIN: f32 = 32.0;

impl State {
    pub(in crate::workspace) fn field_addition_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(status) = &self.field_addition_status else {
            self.field_addition_review_dialog(context, actions);
            return;
        };
        let identity = status.identity;
        let mut command = None;
        let response = Modal::new(Id::new("field_addition_save_status")).show(context, |ui| {
            ui.set_max_width(
                MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            ui.heading(tr(
                ui,
                self.field_addition_declaration()
                    .map_or("datasheet-field-title", |declaration| {
                        super::field_label_key(declaration.kind())
                    }),
            ));
            if let Some((tab, name, _)) = self.field_addition_name() {
                if let Some(table) = super::table_name(tab) {
                    ui.label(table);
                }
                ui.label(name);
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(MAX_REPORT_HEIGHT)
                .show(ui, |ui| {
                    if let Some(outcome) = &status.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    match status.phase {
                        StatusPhase::Saving {
                            cancelled,
                            resolving,
                        } => save_display::pending(ui, cancelled, resolving),
                        StatusPhase::Reopening { cancelled } => {
                            ui.spinner();
                            ui.label(tr(ui, "datasheet-field-reopening"));
                            if cancelled {
                                ui.label(tr(ui, "saved-query-read-cancellation-requested"));
                            }
                        }
                        StatusPhase::Finished | StatusPhase::ReopenFailed => {}
                    }
                    if status.omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                    if let Some(message) = &status.error {
                        diagnostics::show(ui, ui.id().with("field_addition_reopen_error"), message);
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if matches!(
                    status.phase,
                    StatusPhase::Saving {
                        cancelled: false,
                        ..
                    } | StatusPhase::Reopening { cancelled: false }
                ) && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                {
                    command = Some(Command::Cancel);
                }
                if self.can_resolve_field_addition_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_field_addition_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_field_addition_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if self.can_retry_field_addition_reopen(identity)
                    && ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked()
                {
                    command = Some(Command::RetryReopen);
                }
                if self.can_return_to_field_addition_draft(identity)
                    && ui.button(tr(ui, "datasheet-field-return-draft")).clicked()
                {
                    command = Some(Command::ReturnToDraft);
                }
            });
        });
        if command.is_none() && response.should_close() {
            command = match status.phase {
                StatusPhase::Saving {
                    cancelled: false, ..
                }
                | StatusPhase::Reopening { cancelled: false } => Some(Command::Cancel),
                StatusPhase::Finished if self.can_acknowledge_field_addition_save(identity) => {
                    Some(Command::Acknowledge)
                }
                StatusPhase::ReopenFailed => Some(Command::ReturnToDraft),
                _ => None,
            };
        }
        if let Some(command) = command
            && (command != Command::Cancel || self.cancel_field_addition_operation(identity))
        {
            actions.push(Action::FieldAdditionSave { identity, command });
        }
    }

    fn field_addition_review_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some((_, _, _, super::Phase::Reviewing { request_id })) = self.field_addition.draft()
        else {
            return;
        };
        let response = Modal::new(Id::new("field_addition_review")).show(context, |ui| {
            ui.heading(tr(
                ui,
                self.field_addition_declaration()
                    .map_or("datasheet-field-title", |declaration| {
                        super::field_label_key(declaration.kind())
                    }),
            ));
            ui.spinner();
            ui.label(tr(ui, "datasheet-field-reviewing"));
            ui.button(tr(ui, "editing-request-cancellation")).clicked()
        });
        if response.inner || response.should_close() {
            actions.push(Action::CancelFieldAdditionReview { request_id });
        }
    }

    pub(in crate::workspace) fn field_addition_inspection_status(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) {
        let Some(tab) = self.active_object() else {
            return;
        };
        let Some((pending, available, error)) = self.field_addition_inspection(&tab) else {
            return;
        };
        if pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(ui, "datasheet-field-inspecting"));
            });
            return;
        }
        if let Some(message) = error {
            if message.is_empty() {
                ui.label(tr(ui, "objects-memory-limit"));
            } else {
                diagnostics::show(ui, ui.id().with("field_addition_source_error"), message);
            }
        }
        let interrupted = self.field_addition_inspection_interrupted(&tab);
        if interrupted {
            ui.label(tr(ui, "datasheet-field-inspection-interrupted"));
        }
        if (error.is_some() || available == Some(false) || interrupted)
            && !self.busy
            && !self.field_addition_open()
            && ui
                .button(tr(ui, "datasheet-field-retry-inspection"))
                .clicked()
        {
            actions.push(Action::RetryFieldAdditionInspection { tab });
        }
    }
}
