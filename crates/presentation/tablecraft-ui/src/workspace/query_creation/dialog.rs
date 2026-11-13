//! Save-name is a compact visual adaptation; native validation stays at submission.
use super::save::{Phase, QueryCreationSaveCommand as Command};
use crate::localization::tr;
use crate::workspace::{Action, State, diagnostics, query::parameters::copy_text, save_display};
use egui::{Context, Id, Modal, TextEdit, Ui};
use tablecraft_builders::query::MAX_DESIGN_TEXT_BYTES;

impl State {
    pub(in crate::workspace) fn query_creation_save_as_button(&mut self, ui: &mut Ui) {
        if ui
            .add_enabled(
                self.can_begin_query_creation_save_as(),
                egui::Button::new(tr(ui, "query-save-as")),
            )
            .clicked()
        {
            self.queue_query_creation_save_as();
        }
    }

    pub(in crate::workspace) fn query_creation_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if self.query_creation.name.is_some() {
            self.query_creation_name_dialog(context, actions);
        } else {
            self.query_creation_status_dialog(context, actions);
        }
    }

    fn query_creation_name_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some(length) = self
            .query_creation
            .name
            .as_ref()
            .map(|name| name.text.len())
        else {
            return;
        };
        if !self.admit_object_bytes(Some(length)) {
            return;
        }
        // NO_CLONE: The text widget edits a candidate while the prior name is
        // retained for whole-input rollback if its actual allocation is refused.
        let Some(mut text) = self
            .query_creation
            .name
            .as_ref()
            .and_then(|name| copy_text(&name.text))
        else {
            return;
        };
        let initial_focus = self
            .query_creation
            .name
            .as_ref()
            .is_some_and(|name| !name.focus_requested);
        let mut submit = false;
        let mut cancel = false;
        let response = Modal::new(Id::new("query_creation_name")).show(context, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr(ui, "query-save-as"));
            if let Some(message) = &self.error {
                diagnostics::show(ui, ui.id().with("query_creation_name_error"), message);
            }
            let label = ui.label(tr(ui, "query-save-name"));
            let id = ui.make_persistent_id("query_creation_name_text");
            crate::text_input::reject_oversized_events(ui, id, &text, MAX_DESIGN_TEXT_BYTES);
            let had_focus = ui.memory(|memory| memory.has_focus(id));
            let input = ui
                .add(TextEdit::singleline(&mut text).id(id).desired_width(320.0))
                .labelled_by(label.id);
            if initial_focus {
                input.request_focus();
            }
            submit = had_focus
                && ui.input(|input| {
                    input.key_pressed(egui::Key::Enter) && input.modifiers.is_none()
                });
            ui.horizontal(|ui| {
                submit |= ui
                    .add_enabled(
                        !text.trim().is_empty(),
                        egui::Button::new(tr(ui, "query-save-ok")),
                    )
                    .clicked();
                cancel = ui.button(tr(ui, "creation-cancel")).clicked();
            });
        });
        if let Some(name) = &mut self.query_creation.name {
            name.focus_requested = true;
        }
        if response.should_close() || cancel {
            self.cancel_query_creation_name();
            return;
        }
        if !self.set_query_creation_name(text) {
            return;
        }
        if submit && let Some(action) = self.submit_query_creation_name() {
            actions.push(action);
        }
    }

    fn query_creation_status_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let Some(status) = &self.query_creation.save else {
            return;
        };
        let identity = status.identity;
        let recover = self.can_resolve_query_creation_save(identity, true);
        let release = self.can_resolve_query_creation_save(identity, false);
        let acknowledge = self.can_acknowledge_query_creation_save(identity);
        let mut command = None;
        let response = Modal::new(Id::new("query_creation_status")).show(context, |ui| {
            ui.set_max_width(520.0);
            ui.heading(tr(ui, "query-title"));
            ui.label(&status.request.destination);
            if let Some(outcome) = &status.outcome {
                save_display::outcome(ui, outcome);
            }
            match status.phase {
                Phase::Preparing | Phase::Reopening { .. } => {
                    ui.spinner();
                    ui.label(tr(ui, "saved-query-working"));
                }
                Phase::Saving { cancelled } => save_display::pending(ui, cancelled, false),
                Phase::Finished | Phase::ReopenFailed => {}
            }
            if let Some(message) = &status.reopen_error {
                diagnostics::show(ui, ui.id().with("query_creation_reopen_error"), message);
            }
            ui.horizontal_wrapped(|ui| {
                if matches!(
                    status.phase,
                    Phase::Saving { cancelled: false } | Phase::Reopening { cancelled: false }
                ) && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                {
                    command = Some(Command::Cancel);
                }
                if recover && ui.button(tr(ui, "editing-recover")).clicked() {
                    command = Some(Command::Recover);
                }
                if release && ui.button(tr(ui, "editing-release")).clicked() {
                    command = Some(Command::Release);
                }
                if acknowledge && ui.button(tr(ui, "backstage-close")).clicked() {
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
            command = if acknowledge {
                Some(Command::Acknowledge)
            } else {
                None
            };
        }
        if let Some(command) = command
            && let Some(action) = self.request_query_creation_save_command(identity, command)
        {
            actions.push(action);
        }
    }
}
