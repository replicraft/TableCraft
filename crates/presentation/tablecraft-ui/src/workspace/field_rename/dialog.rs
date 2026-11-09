//! Bounded rename prompts adapted to the existing application dialog style.
//! No retained Access capture qualifies these popup/status appearances.
use super::{
    FieldRenameCloseDecision as Decision, FieldRenameSaveCommand as Command,
    FieldRenameSaveIdentity,
    model::{Observation, Phase},
};
use crate::{
    Action, FluentArgs, State,
    localization::{message, tr},
    workspace::{diagnostics, save_display, schema_save::StatusPhase},
};
use egui::{Context, Id, Modal, Ui};

const MAX_WIDTH: f32 = 520.0;
const MAX_REPORT_HEIGHT: f32 = 230.0;
const VIEWPORT_MARGIN: f32 = 32.0;

impl State {
    pub(in crate::workspace) fn field_rename_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(close) = &self.field_rename.close else {
            return;
        };
        let crate::ObjectKind::Table { name: table } = &close.target.tab.target.kind else {
            return;
        };
        let current = self.field_rename_close_current();
        let can_save = self.can_save_field_rename_close();
        let mut choice = None;
        let response = Modal::new(Id::new("field_rename_close")).show(context, |ui| {
            ui.set_max_width(
                MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            ui.heading(tr(ui, "field-rename-close-title"));
            if self.field_rename_source_stale() {
                ui.label(tr(ui, "field-rename-close-confirm"));
            } else {
                ui.label(message(
                    ui,
                    "field-rename-close-question",
                    &[
                        ("table", table.as_str().into()),
                        ("column", close.target.column.as_str().into()),
                    ],
                ));
            }
            if !current {
                ui.label(tr(ui, "field-rename-close-stale"));
            } else if !can_save && !self.field_rename_source_stale() {
                ui.label(tr(ui, "field-rename-close-unavailable"));
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(tr(ui, "field-rename-close-save")),
                    )
                    .clicked()
                {
                    choice = Some(Decision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(ui, "field-rename-close-discard")),
                    )
                    .clicked()
                {
                    choice = Some(Decision::Discard);
                }
                if ui.button(tr(ui, "field-rename-close-cancel")).clicked() {
                    choice = Some(Decision::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(Decision::Cancel);
        }
        if let Some(choice) = choice
            && let Some(result) = self.choose_field_rename_close(choice)
        {
            actions.push(Action::FieldRenameClose { result });
        }
    }

    pub(in crate::workspace) fn field_rename_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(status) = &self.field_rename.status else {
            self.field_rename_review_dialog(context, actions);
            return;
        };
        let identity = status.identity;
        let unchanged = matches!(identity, FieldRenameSaveIdentity::UnchangedReview { .. });
        let mut command = None;
        let response = Modal::new(Id::new("field_rename_save_status")).show(context, |ui| {
            ui.set_max_width(
                MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            ui.heading(tr(ui, "field-rename-title"));
            if let Some((target, proposed, _)) = self.field_rename_proposal() {
                ui.label(message(
                    ui,
                    "field-rename-original",
                    &[("column", target.column.as_str().into())],
                ));
                ui.label(message(
                    ui,
                    "field-rename-proposed",
                    &[("column", proposed.into())],
                ));
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(MAX_REPORT_HEIGHT)
                .show(ui, |ui| {
                    if unchanged {
                        ui.label(tr(ui, "field-rename-unchanged"));
                    } else if let Some(outcome) = &status.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    match status.phase {
                        StatusPhase::Saving {
                            cancelled,
                            resolving,
                        } => save_display::pending(ui, cancelled, resolving),
                        StatusPhase::Reopening { cancelled } => {
                            ui.spinner();
                            ui.label(tr(
                                ui,
                                if unchanged {
                                    "field-rename-unchanged-reopening"
                                } else {
                                    "field-rename-reopening"
                                },
                            ));
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
                        diagnostics::show(ui, ui.id().with("field_rename_reopen_error"), error);
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
                if self.can_resolve_field_rename_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_field_rename_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_field_rename_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if self.can_retry_field_rename_reopen(identity)
                    && ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked()
                {
                    command = Some(Command::RetryReopen);
                }
                if self.can_return_to_field_rename_draft(identity)
                    && ui.button(tr(ui, "field-rename-return")).clicked()
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
                StatusPhase::Finished if self.can_acknowledge_field_rename_save(identity) => {
                    Some(Command::Acknowledge)
                }
                StatusPhase::ReopenFailed => Some(Command::ReturnToDraft),
                _ => None,
            };
        }
        if let Some(command) = command {
            actions.push(Action::FieldRenameSave { identity, command });
        }
    }

    fn field_rename_review_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some(draft) = &self.field_rename.draft else {
            return;
        };
        let Phase::Reviewing { request_id } = draft.phase else {
            return;
        };
        let response = Modal::new(Id::new("field_rename_review")).show(context, |ui| {
            ui.heading(tr(ui, "field-rename-title"));
            ui.spinner();
            ui.label(tr(ui, "field-rename-reviewing"));
            ui.button(tr(ui, "editing-request-cancellation")).clicked()
        });
        if response.inner || response.should_close() {
            actions.push(Action::CancelFieldRenameReview { request_id });
        }
    }

    pub(in crate::workspace) fn field_rename_source_status(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        if self.field_rename.status.is_some() {
            return;
        }
        if self.field_rename_source_stale() {
            self.field_rename_warning_status(ui, actions);
            return;
        }
        let Some(draft) = &self.field_rename.draft else {
            return;
        };
        if draft.phase != Phase::Naming {
            return;
        }
        match &draft.observation {
            Observation::Pending { .. } => {
                ui.spinner();
                ui.label(tr(ui, "field-rename-inspecting"));
            }
            Observation::Eligible { units } => {
                let mut arguments = FluentArgs::new();
                arguments.set("units", *units as i64);
                ui.small(self.localizer.format("field-rename-profile", &arguments));
            }
            Observation::Unsupported => {
                ui.label(tr(ui, "field-rename-source-unavailable"));
            }
            Observation::Failed { message } => {
                if message.is_empty() {
                    ui.label(tr(ui, "objects-memory-limit"));
                } else {
                    diagnostics::show(ui, ui.id().with("field_rename_source_error"), message);
                }
            }
            Observation::Interrupted => {
                ui.label(tr(ui, "field-rename-inspection-interrupted"));
            }
        }
        let retry = matches!(
            draft.observation,
            Observation::Unsupported | Observation::Failed { .. } | Observation::Interrupted
        );
        if retry && !self.busy && ui.button(tr(ui, "field-rename-retry-inspection")).clicked() {
            let target = draft.target.copy();
            if let Some(target) = target {
                // The post-widget collector issues the one prepared readonly request.
                self.retry_field_rename_inspection(&target);
            }
        }
        if ui.button(tr(ui, "field-rename-cancel")).clicked() {
            actions.push(Action::CloseFieldRenameEditor);
        }
    }

    fn field_rename_warning_status(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(warning) = self.field_rename_warning() else {
            return;
        };
        let unchanged = matches!(
            warning.identity,
            FieldRenameSaveIdentity::UnchangedReview { .. }
        );
        egui::ScrollArea::vertical()
            .id_salt("field_rename_warning")
            .max_height(MAX_REPORT_HEIGHT)
            .show(ui, |ui| {
                ui.group(|ui| {
                    ui.label(tr(
                        ui,
                        if unchanged {
                            "field-rename-warning-unchanged"
                        } else {
                            "field-rename-warning-saved"
                        },
                    ));
                    ui.label(message(
                        ui,
                        "field-rename-proposed",
                        &[("column", warning.proposed_name.into())],
                    ));
                    if !unchanged {
                        if let Some(outcome) = warning.outcome {
                            save_display::outcome(ui, outcome);
                        }
                        if warning.cleanup == Some(crate::SaveCleanupNotice::RetainedPrivateJournal)
                        {
                            ui.label(tr(ui, "editing-save-retained-private-journal"));
                        }
                    }
                    if let Some(error) = warning.reopen_error {
                        diagnostics::show(ui, ui.id().with("field_rename_warning_error"), error);
                    }
                    if warning.report_omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                });
            });
        if self.field_rename_open() && ui.button(tr(ui, "field-rename-close-editor")).clicked() {
            actions.push(Action::CloseFieldRenameEditor);
        }
    }
}
