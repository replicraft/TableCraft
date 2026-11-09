//! Presentation of runtime-owned edit decisions and retained save outcomes.
use super::{EditDialog, EditSaveOutcome, EditTarget};
use crate::localization::{for_context, message, tr};
use crate::{
    Cell, datasheet,
    workspace::{Action, State},
};

// Reserve the fixed title and action rows before scrolling variable host content.
const MODAL_VERTICAL_RESERVE: f32 = 180.0;
const MIN_BODY_HEIGHT: f32 = 120.0;

impl State {
    pub(in crate::workspace) fn edit_dialog(
        &mut self,
        ctx: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(dialog) = &mut self.edit else {
            return;
        };

        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("offline_edit")).show(ctx, |ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(320.0, 640.0));
            ui.heading(tr(ui, "editing-title"));
            ui.label(tr(ui, "editing-exclusive-help"));
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("edit_body")
                .auto_shrink([false, true])
                .max_height(
                    (ctx.content_rect().height() - MODAL_VERTICAL_RESERVE).max(MIN_BODY_HEIGHT),
                )
                .show(ui, |ui| match dialog {
                    EditDialog::Checking { target, error, .. } => {
                        target_label(ui, target);
                        if let Some(error) = error {
                            ui.label(error.as_str());
                        } else {
                            ui.spinner();
                            ui.label(tr(ui, "editing-checking"));
                        }
                    }
                    EditDialog::Draft {
                        draft,
                        input,
                        pending,
                        review,
                        reviewed_input,
                        error,
                        ..
                    } => {
                        target_label(ui, &draft.target);
                        ui.label(tr(ui, "editing-original"));
                        stored_cell(ui, &draft.original);

                        ui.label(draft.input_hint.label(&for_context(ui.ctx())));
                        let label = ui.label(tr(ui, "editing-replacement"));
                        let text = ui
                            .add_enabled(
                                !input.is_null,
                                egui::TextEdit::multiline(&mut input.text).desired_rows(3),
                            )
                            .labelled_by(label.id);
                        crate::theme::input_border(ui, &text);
                        let null = ui.add_enabled(
                            draft.nullable,
                            egui::Checkbox::new(&mut input.is_null, tr(ui, "editing-use-null")),
                        );

                        if text.changed() || null.changed() {
                            *pending = None;
                            *review = None;
                            *reviewed_input = None;
                            *error = None;
                        }

                        if let Some(error) = error {
                            ui.label(error.as_str());
                        }
                        if pending.is_some() {
                            ui.spinner();
                            ui.label(tr(ui, "editing-validating"));
                        }

                        if let Some(review) = review {
                            ui.separator();
                            ui.label(tr(ui, "editing-reviewed"));
                            ui.label(tr(ui, "editing-before"));
                            stored_cell(ui, &review.before);
                            ui.label(tr(ui, "editing-after"));
                            stored_cell(ui, &review.after);
                            ui.label(message(
                                ui,
                                "editing-changed-rows",
                                &[("count", review.changed_rows.to_string())],
                            ));
                        }
                    }
                    EditDialog::Saving {
                        cancel_requested,
                        resolving,
                        ..
                    } => {
                        ui.spinner();
                        ui.label(tr(
                            ui,
                            if *resolving {
                                "editing-resolving"
                            } else {
                                "editing-applying"
                            },
                        ));

                        if *cancel_requested {
                            ui.label(tr(ui, "editing-cancellation-requested"));
                        }
                    }
                    EditDialog::Finished { outcome, .. } => match outcome {
                        EditSaveOutcome::Retained {
                            message,
                            can_recover,
                            can_release,
                        } => {
                            ui.label(message.as_str());
                            ui.label(tr(ui, "editing-recovery-owned"));

                            if !*can_recover && !*can_release {
                                ui.label(tr(ui, "editing-diagnosis-required"));
                            }
                        }
                        EditSaveOutcome::Saved(message) => {
                            terminal_outcome(ui, "editing-outcome-saved", message);
                        }
                        EditSaveOutcome::Cancelled(message) => {
                            terminal_outcome(ui, "editing-outcome-cancelled", message);
                        }
                        EditSaveOutcome::Failed(message) => {
                            terminal_outcome(ui, "editing-outcome-failed", message);
                        }
                    },
                });

            ui.horizontal_wrapped(|ui| match dialog {
                EditDialog::Checking { .. } => {
                    close = ui.button(tr(ui, "shell-cancel")).clicked();
                }
                EditDialog::Draft {
                    draft,
                    input,
                    pending,
                    review,
                    ..
                } => {
                    if let Some(review) = review
                        && ui.button(tr(ui, "editing-apply")).clicked()
                    {
                        actions.push(Action::ApplyEdit {
                            review_id: review.id,
                        });
                    }
                    if ui
                        .add_enabled(
                            pending.is_none(),
                            egui::Button::new(tr(ui, "editing-review")),
                        )
                        .clicked()
                    {
                        actions.push(Action::ReviewEdit {
                            draft_id: draft.id,
                            // NO_CLONE: The review action owns the proposed text while the
                            // dialog keeps the entered value for rendering/retry; the
                            // consuming native review captures that input.
                            input: input.clone(),
                        });
                    }
                    close = ui.button(tr(ui, "shell-cancel")).clicked();
                }
                EditDialog::Saving {
                    save_id,
                    cancel_requested,
                    ..
                } => {
                    if !*cancel_requested
                        && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                    {
                        *cancel_requested = true;
                        actions.push(Action::CancelEditSave { save_id: *save_id });
                    }
                }
                EditDialog::Finished { save_id, outcome } => match outcome {
                    EditSaveOutcome::Retained {
                        can_recover,
                        can_release,
                        ..
                    } => {
                        if *can_recover && ui.button(tr(ui, "editing-recover")).clicked() {
                            actions.push(Action::RecoverEditSave { save_id: *save_id });
                        }
                        if *can_release && ui.button(tr(ui, "editing-release")).clicked() {
                            actions.push(Action::ReleaseEditSave { save_id: *save_id });
                        }
                    }
                    _ => close = ui.button(tr(ui, "backstage-close")).clicked(),
                },
            });
        });

        if response.should_close() {
            if let Some(EditDialog::Saving {
                save_id,
                cancel_requested,
                ..
            }) = &mut self.edit
            {
                if !*cancel_requested {
                    *cancel_requested = true;
                    actions.push(Action::CancelEditSave { save_id: *save_id });
                }
            } else {
                close = true;
            }
        }

        if close && !self.edit_save_pending() {
            match self.edit.take() {
                Some(EditDialog::Checking { request_id, .. }) => actions.push(Action::CloseEdit {
                    request_id,
                    draft_id: None,
                }),
                Some(EditDialog::Draft {
                    request_id, draft, ..
                }) => actions.push(Action::CloseEdit {
                    request_id,
                    draft_id: Some(draft.id),
                }),
                Some(EditDialog::Finished { save_id, .. }) => {
                    actions.push(Action::AcknowledgeEditSave { save_id })
                }
                _ => {}
            }
        }
    }
}

fn target_label(ui: &mut egui::Ui, target: &EditTarget) {
    let row = (u64::from(target.parent_ordinal) + 1).to_string();
    ui.label(message(
        ui,
        "editing-target",
        &[
            ("table", target.table.as_str()),
            ("column", target.column.as_str()),
            ("row", row.as_str()),
        ],
    ));
}

fn stored_cell(ui: &mut egui::Ui, value: &Cell) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 26.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let _ = datasheet::cell(ui, value, &tr(ui, "editing-stored-value"), 1, true);
        },
    );
}

fn terminal_outcome(ui: &mut egui::Ui, heading: &str, message: &str) {
    ui.heading(tr(ui, heading));
    ui.label(message);
}
