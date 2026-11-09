use super::{body_height, size};
use crate::{
    Action, State,
    localization::tr,
    workspace::{
        diagnostics,
        field_drop::{FieldDropSaveCommand as Command, model::Phase},
        save_display,
        schema_save::StatusPhase,
    },
};
use egui::{Context, Id, Modal};

impl State {
    pub(in crate::workspace) fn field_drop_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(status) = &self.field_drop.status else {
            self.field_drop_review_dialog(context, actions);
            return;
        };
        let identity = status.identity;
        let mut command = None;
        let response = Modal::new(Id::new("field_drop_save_status")).show(context, |ui| {
            size(ui, context);
            ui.heading(tr(ui, "field-drop-title"));
            egui::ScrollArea::vertical()
                .max_height(body_height(context))
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
                            ui.label(tr(ui, "field-drop-reopening"));
                            if cancelled {
                                ui.label(tr(ui, "saved-query-read-cancellation-requested"));
                            }
                        }
                        StatusPhase::Finished | StatusPhase::ReopenFailed => {}
                    }
                    if status.omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                    if let Some(error) = &status.error {
                        diagnostics::show(ui, ui.id().with("drop_reopen_error"), error);
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
                if self.can_resolve_field_drop_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_field_drop_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_field_drop_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if self.can_retry_field_drop_reopen(identity)
                    && ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked()
                {
                    command = Some(Command::RetryReopen);
                }
                if self.can_return_to_field_drop_draft(identity)
                    && ui.button(tr(ui, "field-drop-return")).clicked()
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
                StatusPhase::Finished if self.can_acknowledge_field_drop_save(identity) => {
                    Some(Command::Acknowledge)
                }
                StatusPhase::ReopenFailed => Some(Command::ReturnToDraft),
                _ => None,
            };
        }
        if let Some(command) = command {
            actions.push(Action::FieldDropSave { identity, command });
        }
    }

    fn field_drop_review_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some(draft) = &self.field_drop.draft else {
            return;
        };
        let Phase::Reviewing { request_id } = draft.phase else {
            return;
        };
        let response = Modal::new(Id::new("field_drop_review")).show(context, |ui| {
            size(ui, context);
            ui.heading(tr(ui, "field-drop-title"));
            ui.spinner();
            ui.label(tr(ui, "field-drop-reviewing"));
            ui.button(tr(ui, "editing-request-cancellation")).clicked()
        });
        if response.inner || response.should_close() {
            actions.push(Action::CancelFieldDropReview { request_id });
        }
    }
}
