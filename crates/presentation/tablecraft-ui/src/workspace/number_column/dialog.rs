//! Number-schema owner status; it never routes through a cell-edit dialog.
use super::{NumberColumnSaveCommand as Command, status::StatusPhase};
use crate::workspace::{diagnostics, save_display};
use crate::{Action, State, localization::tr};
use egui::{Context, Id, Modal};

const MAX_WIDTH: f32 = 520.0;
const MAX_REPORT_HEIGHT: f32 = 240.0;
const VIEWPORT_MARGIN: f32 = 32.0;

impl State {
    pub(in crate::workspace) fn number_column_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(status) = &self.number_column_status else {
            self.number_column_review_dialog(context, actions);
            return;
        };
        let identity = status.identity;
        let mut command = None;
        let response = Modal::new(Id::new("number_column_save_status")).show(context, |ui| {
            ui.set_max_width(
                MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            ui.heading(tr(ui, "table-draft-number"));
            if let Some((tab, name, _)) = self.number_column_name() {
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
                            ui.label(tr(ui, "datasheet-number-reopening"));
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
                        diagnostics::show(ui, ui.id().with("number_column_reopen_error"), message);
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
                if self.can_resolve_number_column_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_number_column_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_number_column_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if self.can_retry_number_column_reopen(identity)
                    && ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked()
                {
                    command = Some(Command::RetryReopen);
                }
            });
        });
        if command.is_none() && response.should_close() {
            command = match status.phase {
                StatusPhase::Saving {
                    cancelled: false, ..
                }
                | StatusPhase::Reopening { cancelled: false } => Some(Command::Cancel),
                StatusPhase::Finished if self.can_acknowledge_number_column_save(identity) => {
                    Some(Command::Acknowledge)
                }
                _ => None,
            };
        }
        if let Some(command) = command {
            if command != Command::Cancel || self.cancel_number_column_operation(identity) {
                actions.push(Action::NumberColumnSave { identity, command });
            }
        }
    }

    fn number_column_review_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some((_, _, _, super::Phase::Reviewing { request_id })) = self.number_column.draft()
        else {
            return;
        };
        let response = Modal::new(Id::new("number_column_review")).show(context, |ui| {
            ui.heading(tr(ui, "table-draft-number"));
            ui.spinner();
            ui.label(tr(ui, "datasheet-number-reviewing"));
            ui.button(tr(ui, "editing-request-cancellation")).clicked()
        });
        if response.inner || response.should_close() {
            actions.push(Action::CancelNumberColumnReview { request_id });
        }
    }

    pub(in crate::workspace) fn number_column_inspection_status(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) {
        let Some(tab) = self.active_object() else {
            return;
        };
        let Some((pending, available, error)) = self.number_column_inspection(&tab) else {
            return;
        };
        if pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(ui, "datasheet-number-inspecting"));
            });
            return;
        }
        if let Some(message) = error {
            if message.is_empty() {
                ui.label(tr(ui, "objects-memory-limit"));
            } else {
                diagnostics::show(ui, ui.id().with("number_source_error"), message);
            }
        }
        if (error.is_some() || available == Some(false))
            && !self.busy
            && !self.number_column_open()
            && ui
                .button(tr(ui, "datasheet-number-retry-inspection"))
                .clicked()
        {
            actions.push(Action::RetryNumberColumnInspection { tab });
        }
    }
}
