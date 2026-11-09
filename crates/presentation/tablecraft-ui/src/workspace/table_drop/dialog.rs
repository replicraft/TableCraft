//! Navigation deletion follows documented confirmation; no retained dialog capture exists.
use super::{
    TableDropSaveCommand,
    model::{Observation, Phase},
};
use crate::{
    Action, State,
    localization::{message, tr},
    workspace::{EditSaveOutcome, diagnostics, save_display, schema_save::StatusPhase},
};
use egui::{Context, Id, Modal, Ui};
use std::sync::Arc;

const DIALOG_WIDTH: f32 = 460.0;
const VIEWPORT_MARGIN: f32 = 32.0;
const MIN_DIALOG_WIDTH: f32 = 100.0;
const FOOTER_ALLOWANCE: f32 = 150.0;

impl State {
    pub(in crate::workspace) fn table_drop_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(session) = self
            .table_drop
            .session
            .as_ref()
            .filter(|session| session.visible)
        else {
            return;
        };
        let current = self.table_drop_binding(&session.request.target);
        let can_yes = current
            && session.phase == Phase::Confirming
            && matches!(session.observation, Observation::Eligible { .. });
        let can_retry = current
            && !self.busy
            && session.phase == Phase::Confirming
            && matches!(
                session.observation,
                Observation::Unsupported | Observation::Interrupted | Observation::Failed(_)
            );
        let mut choice = None;
        let mut retry = false;
        let mut cancel_review = false;
        let Some(session) = self.table_drop.session.as_mut() else {
            return;
        };
        let response = Modal::new(Id::new("table_drop")).show(context, |ui| {
            ui.set_max_width(
                DIALOG_WIDTH
                    .min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(MIN_DIALOG_WIDTH)),
            );
            ui.heading(tr(ui, "table-drop-title"));
            egui::ScrollArea::vertical()
                .max_height((context.viewport_rect().height() - FOOTER_ALLOWANCE).max(0.0))
                .show(ui, |ui| {
                    ui.label(message(
                        ui,
                        "table-drop-confirm",
                        &[("table", session.request.table_name().unwrap_or(""))],
                    ));
                    match session.phase {
                        Phase::Confirming => {
                            ui.label(tr(ui, "table-drop-data-loss"));
                            match &session.observation {
                                Observation::Unrequested
                                | Observation::Reserved
                                | Observation::Pending(_) => {
                                    ui.spinner();
                                    ui.label(tr(ui, "table-drop-inspecting"));
                                }
                                Observation::Eligible { declared_rows } => {
                                    let count = declared_rows.to_string();
                                    ui.label(message(
                                        ui,
                                        "table-drop-records",
                                        &[("count", count.as_str())],
                                    ));
                                }
                                Observation::Unsupported => {
                                    ui.label(tr(ui, "table-drop-unavailable"));
                                }
                                Observation::Interrupted => {
                                    ui.label(tr(ui, "table-drop-interrupted"));
                                }
                                Observation::Failed(error) if error.is_empty() => {
                                    ui.label(tr(ui, "objects-memory-limit"));
                                }
                                Observation::Failed(error) => {
                                    diagnostics::show(ui, ui.id().with("inspection_error"), error);
                                }
                            }
                        }
                        Phase::Reviewing { .. } => {
                            ui.spinner();
                            ui.label(tr(ui, "table-drop-reviewing"));
                        }
                        Phase::Owned(_) => {
                            if let Some(status) = &session.status {
                                match status.phase {
                                    StatusPhase::Saving {
                                        cancelled,
                                        resolving,
                                    } => {
                                        save_display::pending(ui, cancelled, resolving);
                                    }
                                    StatusPhase::Reopening { .. } => {
                                        ui.spinner();
                                        ui.label(tr(ui, "table-drop-reopening"));
                                    }
                                    StatusPhase::Finished | StatusPhase::ReopenFailed => {
                                        if let Some(outcome) = &status.outcome {
                                            save_display::outcome(ui, outcome);
                                        }
                                        if status.phase == StatusPhase::ReopenFailed {
                                            ui.label(tr(ui, "table-drop-reopen-failed"));
                                        }
                                        if let Some(error) = &status.error {
                                            diagnostics::show(
                                                ui,
                                                ui.id().with("reopen_error"),
                                                error,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(cleanup) = session.cleanup {
                        match cleanup {
                            crate::workspace::SaveCleanupNotice::RetainedPrivateJournal => {
                                ui.label(tr(ui, "editing-save-retained-private-journal"));
                            }
                        }
                    }
                });
            ui.horizontal_wrapped(|ui| match session.phase {
                Phase::Confirming => {
                    let yes = ui.add_enabled(can_yes, egui::Button::new(tr(ui, "table-drop-yes")));
                    if can_yes && session.focus_yes {
                        yes.request_focus();
                    }
                    if yes.clicked() {
                        choice = Some(true);
                    }
                    let no = ui.button(tr(ui, "table-drop-no"));
                    if !can_yes && session.focus_yes {
                        no.request_focus();
                    }
                    if no.clicked() {
                        choice = Some(false);
                    }
                    if can_retry && ui.button(tr(ui, "table-drop-retry-inspection")).clicked() {
                        retry = true;
                    }
                }
                Phase::Reviewing { .. } => {
                    if ui
                        .add_enabled(
                            !session.cancel_review_requested,
                            egui::Button::new(tr(ui, "shell-cancel")),
                        )
                        .clicked()
                    {
                        cancel_review = true;
                    }
                }
                Phase::Owned(identity) => {
                    let Some(status) = session.status.as_ref() else {
                        return;
                    };
                    let command = match status.phase {
                        StatusPhase::Saving { cancelled, .. }
                        | StatusPhase::Reopening { cancelled } => (!cancelled
                            && ui.button(tr(ui, "editing-request-cancellation")).clicked())
                        .then_some(TableDropSaveCommand::Cancel),
                        StatusPhase::ReopenFailed => {
                            if ui.button(tr(ui, "table-drop-retry-reopen")).clicked() {
                                Some(TableDropSaveCommand::RetryReopen)
                            } else if ui.button(tr(ui, "table-drop-return")).clicked() {
                                Some(TableDropSaveCommand::ReturnToCatalog)
                            } else {
                                None
                            }
                        }
                        StatusPhase::Finished => {
                            if let Some(EditSaveOutcome::Retained {
                                can_recover,
                                can_release,
                                ..
                            }) = &status.outcome
                            {
                                if ui
                                    .add_enabled(
                                        *can_recover,
                                        egui::Button::new(tr(ui, "editing-recover")),
                                    )
                                    .clicked()
                                {
                                    Some(TableDropSaveCommand::Recover)
                                } else if ui
                                    .add_enabled(
                                        *can_release,
                                        egui::Button::new(tr(ui, "editing-release")),
                                    )
                                    .clicked()
                                {
                                    Some(TableDropSaveCommand::Release)
                                } else {
                                    None
                                }
                            } else if ui.button(tr(ui, "backstage-close")).clicked() {
                                Some(TableDropSaveCommand::Acknowledge)
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(command) = command {
                        actions.push(Action::TableDropSave { identity, command });
                    }
                }
            });
        });
        session.focus_yes = false;
        if response.should_close() {
            match session.phase {
                Phase::Confirming => choice = Some(false),
                Phase::Reviewing { .. } => cancel_review = true,
                Phase::Owned(identity) => {
                    if session.status.as_ref().is_some_and(|status| {
                        matches!(
                            status.phase,
                            StatusPhase::Saving {
                                cancelled: false,
                                ..
                            } | StatusPhase::Reopening { cancelled: false }
                        )
                    }) {
                        actions.push(Action::TableDropSave {
                            identity,
                            command: TableDropSaveCommand::Cancel,
                        });
                    }
                }
            }
        }
        if choice == Some(false) {
            let Some(session) = self.table_drop.session.take() else {
                return;
            };
            if let Observation::Pending(request_id) = session.observation {
                actions.push(Action::CancelTableDropInspection { request_id });
            }
        } else if choice == Some(true) && can_yes {
            let Some(session) = self.table_drop.session.as_mut() else {
                return;
            };
            session.phase = Phase::Reviewing { read_id: None };
            // NO_CLONE: Confirmation and host keep the same immutable request through fresh
            // review. Arc shares the target allocation; a frame borrow cannot survive the
            // asynchronous result, and independently copying the name adds no isolation.
            let request = Arc::clone(&session.request);
            actions.push(Action::DeleteTable { request });
        } else if retry {
            if let Some(session) = self.table_drop.session.as_mut() {
                session.observation = Observation::Unrequested;
            }
        } else if cancel_review {
            let Some(session) = self.table_drop.session.as_mut() else {
                return;
            };
            match session.phase {
                Phase::Reviewing {
                    read_id: Some(request_id),
                } if !session.cancel_review_requested => {
                    session.cancel_review_requested = true;
                    actions.push(Action::CancelTableDropReview { request_id });
                }
                Phase::Reviewing { read_id: None } => self.table_drop.session = None,
                _ => {}
            }
        }
    }

    pub(in crate::workspace) fn table_drop_warning_status(&mut self, ui: &mut Ui) {
        if !self.table_drop_source_stale() || self.table_drop_open() {
            return;
        }
        let mut details = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(ui, "table-drop-refresh-required"));
            details = ui.button(tr(ui, "table-drop-details")).clicked();
        });
        if details {
            if let Some(session) = self.table_drop.session.as_mut() {
                session.visible = true;
            }
        }
    }
}
