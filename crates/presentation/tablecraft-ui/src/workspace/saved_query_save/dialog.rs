//! A query-owned status modal, separate from row-edit presentation.
use super::{Phase, SavedQuerySqlSaveCommand as Command};
use crate::localization::tr;
use crate::workspace::{diagnostics, save_display};
use crate::{Action, State};
use egui::{Context, Id, Modal};

const MAX_STATUS_WIDTH: f32 = 520.0;
const MAX_REPORT_HEIGHT: f32 = 240.0;

impl State {
    pub(in crate::workspace) fn saved_query_sql_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(status) = &self.saved_query_sql_save else {
            return;
        };
        let identity = status.identity;
        let mut command = None;
        let response = Modal::new(Id::new("saved_query_sql_save_status")).show(context, |ui| {
            ui.set_max_width(MAX_STATUS_WIDTH.min(context.viewport_rect().width() - 32.0));
            ui.heading(tr(ui, "query-title"));
            ui.label(&status.request.target.name);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("query_save_report")
                .max_height(MAX_REPORT_HEIGHT)
                .show(ui, |ui| {
                    if let Some(outcome) = &status.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    match status.phase {
                        Phase::Preparing | Phase::Reopening { .. } => {
                            ui.spinner();
                            ui.label(tr(ui, "saved-query-working"));
                            if matches!(status.phase, Phase::Reopening { cancelled: true }) {
                                ui.label(tr(ui, "editing-cancellation-requested"));
                            }
                        }
                        Phase::Saving {
                            cancelled,
                            resolving,
                        } => {
                            save_display::pending(ui, cancelled, resolving);
                        }
                        Phase::Finished | Phase::ReopenFailed => {}
                    }
                    if status.report_omitted || status.reopen_error_omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                    if let Some(message) = &status.reopen_error {
                        diagnostics::show(ui, ui.id().with("query_reopen_error"), message);
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if matches!(
                    status.phase,
                    Phase::Saving {
                        cancelled: false,
                        ..
                    } | Phase::Reopening { cancelled: false }
                ) && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                {
                    command = Some(Command::Cancel);
                }
                if self.can_resolve_saved_query_sql_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_saved_query_sql_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_saved_query_sql_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if status.phase == Phase::ReopenFailed {
                    if ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked() {
                        command = Some(Command::RetryReopen);
                    }
                    if ui.button(tr(ui, "saved-query-save-return-draft")).clicked() {
                        command = Some(Command::ReturnToDraft);
                    }
                }
            });
        });
        if command.is_none() && response.should_close() {
            command = match status.phase {
                Phase::Saving {
                    cancelled: false, ..
                }
                | Phase::Reopening { cancelled: false } => Some(Command::Cancel),
                Phase::Finished if self.can_acknowledge_saved_query_sql_save(identity) => {
                    Some(Command::Acknowledge)
                }
                Phase::ReopenFailed => Some(Command::ReturnToDraft),
                _ => None,
            };
        }
        if let Some(command) = command
            && let Some(action) = self.request_saved_query_sql_save_command(identity, command)
        {
            actions.push(action);
        }
    }
}
