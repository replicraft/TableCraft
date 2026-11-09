//! Current-database Options adaptation; no matching Access Options capture exists.
use super::{DatabaseOptionsSaveCommand as Command, NameAutoCorrectState, Phase};
use crate::workspace::{diagnostics, save_display};
use crate::{Action, State, localization::tr};
use egui::{Context, Id, Modal, Ui};

const MAX_WIDTH: f32 = 520.0;
const REPORT_HEIGHT: f32 = 220.0;
const VIEWPORT_MARGIN: f32 = 32.0;

impl State {
    pub(in crate::workspace) fn database_options_pane(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        ui.add_space(18.0);
        ui.heading(tr(ui, "database-options-current-database"));
        let Some(revision) = self
            .database
            .as_ref()
            .map(|database| database.document_revision)
        else {
            ui.label(tr(ui, "database-options-no-database"));
            return;
        };
        ui.label(tr(ui, "database-options-name-autocorrect"));
        let stale = self.database_options_source_stale();
        // The retained observation belongs to BEFORE. Once AFTER is known but
        // its fresh read failed, do not present that old value as current.
        if !stale {
            let key = match self.database_options_observation() {
                None => "database-options-state-unknown",
                Some(NameAutoCorrectState::Missing) => "database-options-state-missing",
                Some(NameAutoCorrectState::Absent) => "database-options-state-absent",
                Some(NameAutoCorrectState::ExplicitOff) => "database-options-state-off",
                Some(NameAutoCorrectState::Other) => "database-options-state-other",
            };
            ui.label(tr(ui, key));
            ui.small(tr(ui, "database-options-help"));
        }
        if self.needs_database_options_inspection() {
            actions.push(Action::InspectDatabaseOptions {
                document_revision: revision,
            });
        }
        if matches!(self.database_options.phase, Phase::Inspecting { .. }) {
            ui.spinner();
            ui.label(tr(ui, "database-options-inspecting"));
        }
        self.database_options_warning(ui);
        let enabled = self.can_disable_name_auto_correct();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(tr(ui, "database-options-disable")),
            )
            .on_disabled_hover_text(tr(ui, "database-options-change-unavailable"))
            .clicked()
        {
            self.request_database_options_close_confirmation();
        }
        for key in [
            "database-options-enable-unavailable",
            "database-options-log-unavailable",
        ] {
            ui.add_enabled(false, egui::Button::new(tr(ui, key)))
                .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
        }
        if self.database_options.revision == Some(revision) && !self.database_options_pending() {
            if !stale && let Some(message) = &self.database_options.error {
                diagnostics::show(
                    ui,
                    ui.id().with("database_options_inspection_error"),
                    message,
                );
            }
            if self.database_options.omitted {
                ui.label(tr(ui, "objects-memory-limit"));
            }
            if self.database_options.error.is_some()
                && !self.database_options_source_stale()
                && ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new(tr(ui, "database-options-retry-inspection")),
                    )
                    .clicked()
            {
                self.retry_database_options_inspection();
            }
        }
    }

    pub(in crate::workspace) fn database_options_modal_open(&self) -> bool {
        matches!(
            self.database_options.phase,
            Phase::Confirming
                | Phase::Reviewing { .. }
                | Phase::Reviewed { .. }
                | Phase::Saving { .. }
                | Phase::Finished { .. }
                | Phase::Reopening { .. }
                | Phase::ReopenFailed { .. }
        )
    }

    pub(in crate::workspace) fn database_options_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if self.database_options.phase == Phase::Confirming {
            let response =
                Modal::new(Id::new("database_options_close_objects")).show(context, |ui| {
                    ui.set_max_width(
                        MAX_WIDTH
                            .min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
                    );
                    ui.heading(tr(ui, "database-options-close-title"));
                    ui.label(tr(ui, "database-options-close-message"));
                    let mut arguments = crate::FluentArgs::new();
                    arguments.set("count", self.database_options_close_names().count() as i64);
                    ui.label(
                        self.localizer
                            .format("database-options-close-count", &arguments),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(REPORT_HEIGHT)
                        .show(ui, |ui| {
                            for name in self.database_options_close_names() {
                                ui.label(name);
                            }
                        });
                    ui.horizontal(|ui| {
                        if ui
                            .button(tr(ui, "database-options-close-continue"))
                            .clicked()
                        {
                            return Some(true);
                        }
                        if ui.button(tr(ui, "database-options-close-cancel")).clicked() {
                            return Some(false);
                        }
                        None
                    })
                    .inner
                });
            if response.inner == Some(true) {
                if let Some(request) = self.approve_database_options_close() {
                    actions.push(Action::DisableNameAutoCorrect { request });
                }
            } else if response.inner == Some(false) || response.should_close() {
                self.cancel_database_options_close();
            }
            return;
        }
        if let Phase::Reviewing {
            request_id,
            cancelled,
        } = self.database_options.phase
        {
            let response = Modal::new(Id::new("database_options_review")).show(context, |ui| {
                ui.heading(tr(ui, "database-options-current-database"));
                ui.spinner();
                ui.label(tr(ui, "database-options-reviewing"));
                ui.add_enabled(
                    !cancelled,
                    egui::Button::new(tr(ui, "editing-request-cancellation")),
                )
                .clicked()
            });
            if (response.inner || response.should_close())
                && self.cancel_database_options_review(request_id)
            {
                actions.push(Action::CancelDatabaseOptionsReview { request_id });
            }
            return;
        }
        let Some(identity) = self.database_options.identity() else {
            return;
        };
        if matches!(self.database_options.phase, Phase::Warning { .. }) {
            return;
        }
        let mut command = None;
        let response = Modal::new(Id::new("database_options_save")).show(context, |ui| {
            ui.set_max_width(
                MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            ui.heading(tr(ui, "database-options-current-database"));
            ui.label(tr(ui, "database-options-disable"));
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(REPORT_HEIGHT)
                .show(ui, |ui| {
                    if let Some(outcome) = &self.database_options.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    match self.database_options.phase {
                        Phase::Saving {
                            cancelled,
                            resolving,
                            ..
                        } => save_display::pending(ui, cancelled, resolving),
                        Phase::Reopening { cancelled, .. } => {
                            ui.spinner();
                            ui.label(tr(ui, "database-options-reopening"));
                            if cancelled {
                                ui.label(tr(ui, "saved-query-read-cancellation-requested"));
                            }
                        }
                        _ => {}
                    }
                    if let Some(message) = &self.database_options.error {
                        diagnostics::show(
                            ui,
                            ui.id().with("database_options_reopen_error"),
                            message,
                        );
                    }
                    if self.database_options.omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if matches!(
                    self.database_options.phase,
                    Phase::Saving {
                        cancelled: false,
                        ..
                    } | Phase::Reopening {
                        cancelled: false,
                        ..
                    }
                ) && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                {
                    command = Some(Command::Cancel);
                }
                if self.can_resolve_database_options_save(identity, true)
                    && ui.button(tr(ui, "editing-recover")).clicked()
                {
                    command = Some(Command::Recover);
                }
                if self.can_resolve_database_options_save(identity, false)
                    && ui.button(tr(ui, "editing-release")).clicked()
                {
                    command = Some(Command::Release);
                }
                if self.can_acknowledge_database_options_save(identity)
                    && ui.button(tr(ui, "backstage-close")).clicked()
                {
                    command = Some(Command::Acknowledge);
                }
                if matches!(self.database_options.phase, Phase::ReopenFailed { .. }) {
                    if ui.button(tr(ui, "saved-query-save-retry-reopen")).clicked() {
                        command = Some(Command::RetryReopen);
                    }
                    if ui
                        .button(tr(ui, "database-options-return-settings"))
                        .clicked()
                    {
                        command = Some(Command::ReturnToSettings);
                    }
                }
            });
        });
        if command.is_none() && response.should_close() {
            command = match self.database_options.phase {
                Phase::Saving {
                    cancelled: false, ..
                }
                | Phase::Reopening {
                    cancelled: false, ..
                } => Some(Command::Cancel),
                Phase::Finished { .. } if self.can_acknowledge_database_options_save(identity) => {
                    Some(Command::Acknowledge)
                }
                Phase::ReopenFailed { .. } => Some(Command::ReturnToSettings),
                _ => None,
            };
        }
        if let Some(command) = command
            && (command != Command::Cancel || self.cancel_database_options_operation(identity))
        {
            actions.push(Action::DatabaseOptionsSave { identity, command });
        }
    }

    fn database_options_warning(&mut self, ui: &mut Ui) {
        if !self.database_options_source_stale() {
            return;
        }
        let Phase::Warning { open, .. } = self.database_options.phase else {
            return;
        };
        ui.label(tr(ui, "database-options-saved-warning"));
        if open {
            if let Some(outcome) = &self.database_options.outcome {
                save_display::outcome(ui, outcome);
            }
            if let Some(message) = &self.database_options.error {
                diagnostics::show(ui, ui.id().with("database_options_warning"), message);
            }
            if self.database_options.cleanup.is_some() {
                ui.label(tr(ui, "editing-save-retained-private-journal"));
            }
            if ui.button(tr(ui, "backstage-close")).clicked()
                && let Phase::Warning { open, .. } = &mut self.database_options.phase
            {
                *open = false;
            }
        }
    }
}
