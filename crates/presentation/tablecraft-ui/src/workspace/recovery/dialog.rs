//! No recovery runs merely because this modal is opened or rendered.
use super::{Phase, RecoveryOutcome};
use crate::localization::tr;
use crate::workspace::{Action, State, diagnostics};
use egui::{Id, Key, Modifiers};

const DIALOG_WIDTH: f32 = 520.0;
const VIEWPORT_RESERVE: f32 = 64.0;
const MIN_DIALOG_WIDTH: f32 = 240.0;

impl State {
    pub(in crate::workspace) fn recovery_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let synthetic = self.mock_preview;
        let Some(owner) = &mut self.recovery else {
            return;
        };
        // Escape clears toolkit focus before rendering. The modal alone owns
        // dismissal; running/retained resources never inherit default close.
        let escape = context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        let response =
            egui::Modal::new(Id::new(("restart_recovery", owner.attempt))).show(context, |ui| {
                ui.set_width(
                    (context.content_rect().width() - VIEWPORT_RESERVE)
                        .clamp(MIN_DIALOG_WIDTH, DIALOG_WIDTH),
                );
                ui.heading(tr(ui, "recovery-title"));
                if synthetic {
                    ui.small(tr(ui, "recovery-mock-help"));
                }
                ui.add(egui::Label::new(egui::RichText::new(&owner.path).small()).truncate())
                    .on_hover_text(&owner.path);
                ui.separator();
                if let Some(error) = &owner.error {
                    diagnostics::show(ui, ui.id().with("recovery_request_error"), error);
                }
                match &mut owner.phase {
                    Phase::Prompt => {
                        ui.label(tr(ui, "recovery-explanation"));
                        ui.label(tr(ui, "recovery-exclusive"));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let cancel = ui.button(tr(ui, "recovery-cancel"));
                            if owner.focus_cancel {
                                cancel.request_focus();
                                owner.focus_cancel = false;
                            }
                            if cancel.clicked() || escape {
                                actions.push(Action::CloseRecovery {
                                    attempt_id: owner.attempt,
                                });
                            }
                            if ui.button(tr(ui, "recovery-recover")).clicked() {
                                actions.push(Action::BeginRecovery {
                                    attempt_id: owner.attempt,
                                });
                            }
                        });
                    }
                    Phase::Running {
                        operation,
                        cancel_requested,
                    } => {
                        ui.spinner();
                        ui.label(tr(ui, "recovery-running"));
                        if *cancel_requested {
                            ui.label(tr(ui, "recovery-cancel-requested"));
                        } else if ui.button(tr(ui, "recovery-request-cancel")).clicked() || escape {
                            *cancel_requested = true;
                            actions.push(Action::CancelRecovery {
                                operation_id: *operation,
                            });
                        }
                    }
                    Phase::Outcome { operation, outcome } => {
                        ui.heading(tr(ui, "recovery-result"));
                        match outcome {
                            RecoveryOutcome::Finished(message) => {
                                diagnostics::show(ui, ui.id().with("recovery_report"), message);
                                if ui.button(tr(ui, "recovery-close")).clicked() || escape {
                                    actions.push(Action::AcknowledgeRecovery {
                                        operation_id: *operation,
                                    });
                                }
                            }
                            RecoveryOutcome::Retained {
                                message,
                                can_recover,
                                can_release,
                            } => {
                                diagnostics::show(ui, ui.id().with("recovery_report"), message);
                                ui.label(tr(ui, "recovery-retained"));
                                if !*can_recover && !*can_release {
                                    ui.label(tr(ui, "recovery-diagnosis"));
                                }
                                ui.horizontal_wrapped(|ui| {
                                    if *can_recover
                                        && ui.button(tr(ui, "recovery-recover")).clicked()
                                    {
                                        actions.push(Action::ResolveRecovery {
                                            operation_id: *operation,
                                            recover: true,
                                        });
                                    }
                                    if *can_release
                                        && ui.button(tr(ui, "recovery-release")).clicked()
                                    {
                                        actions.push(Action::ResolveRecovery {
                                            operation_id: *operation,
                                            recover: false,
                                        });
                                    }
                                });
                            }
                        }
                    }
                }
            });
        if response.should_close() && !escape {
            match &mut owner.phase {
                Phase::Prompt => actions.push(Action::CloseRecovery {
                    attempt_id: owner.attempt,
                }),
                Phase::Running {
                    operation,
                    cancel_requested,
                } if !*cancel_requested => {
                    *cancel_requested = true;
                    actions.push(Action::CancelRecovery {
                        operation_id: *operation,
                    });
                }
                _ => {}
            }
        }
    }
}
