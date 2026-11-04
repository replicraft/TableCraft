//! Presentation of runtime-owned edit decisions and retained save outcomes.
use super::{EditDialog, EditSaveOutcome, EditTarget};
use crate::{
    Cell, datasheet,
    workspace::{Action, State},
};

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
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(320.0, 640.0));
            ui.heading("Offline cell edit");
            ui.label("Exclusive offline editing. Close this file in Access and other applications before applying.");
            ui.separator();

            match dialog {
                EditDialog::Checking { target, error, .. } => {
                    target_label(ui, target);
                    if let Some(error) = error {
                        ui.label(error.as_str());
                    } else {
                        ui.spinner();
                        ui.label("Checking this cell and reading its original value…");
                    }
                    close = ui.button("Cancel").clicked();
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
                    ui.label("Original stored value");
                    stored_cell(ui, &draft.original);

                    ui.label(&draft.input_kind);
                    let label = ui.label("Replacement");
                    let text = ui
                        .add_enabled(
                            !input.is_null,
                            egui::TextEdit::multiline(&mut input.text).desired_rows(3),
                        )
                        .labelled_by(label.id);
                    let null = ui.add_enabled(
                        draft.nullable,
                        egui::Checkbox::new(&mut input.is_null, "Use NULL (subject to validation)"),
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
                        ui.label("Validating the complete edit…");
                    }

                    if let Some(review) = review {
                        ui.separator();
                        ui.label("Reviewed change");
                        ui.label("Before");
                        stored_cell(ui, &review.before);
                        ui.label("After");
                        stored_cell(ui, &review.after);
                        ui.label(format!("{} changed row(s)", review.changed_rows));
                        if ui.button("Apply to original file").clicked() {
                            actions.push(Action::ApplyEdit {
                                review_id: review.id,
                            });
                        }
                    }

                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(pending.is_none(), egui::Button::new("Review change"))
                            .clicked()
                        {
                            actions.push(Action::ReviewEdit {
                                draft_id: draft.id,
                                input: input.clone(),
                            });
                        }
                        close = ui.button("Cancel").clicked();
                    });
                }
                EditDialog::Saving {
                    save_id,
                    cancel_requested,
                    resolving,
                } => {
                    ui.spinner();
                    ui.label(if *resolving {
                        "Resolving retained save resources. Keep this window open until its outcome is known."
                    } else {
                        "Applying the reviewed edit. Keep this window open until its outcome is known."
                    });

                    if *cancel_requested {
                        ui.label("Cancellation requested; waiting for the actual save outcome.");
                    } else if ui.button("Request cancellation").clicked() {
                        *cancel_requested = true;
                        actions.push(Action::CancelEditSave {
                            save_id: *save_id,
                        });
                    }
                }
                EditDialog::Finished { save_id, outcome } => {
                    match outcome {
                        EditSaveOutcome::Retained {
                            message,
                            can_recover,
                            can_release,
                        } => {
                            ui.label(message.as_str());
                            ui.label("The save session still owns recovery resources. Resolve it before continuing.");

                            if *can_recover && ui.button("Recover save").clicked() {
                                actions.push(Action::RecoverEditSave {
                                    save_id: *save_id,
                                });
                            }
                            if *can_release && ui.button("Release save resources").clicked() {
                                actions.push(Action::ReleaseEditSave {
                                    save_id: *save_id,
                                });
                            }
                            if !*can_recover && !*can_release {
                                ui.label("Diagnosis is required; automatic recovery and release are unavailable.");
                            }
                        }
                        EditSaveOutcome::Saved(message)
                        | EditSaveOutcome::Cancelled(message)
                        | EditSaveOutcome::Failed(message) => {
                            ui.label(message.as_str());
                            close = ui.button("Close").clicked();
                        }
                    }
                }
            }
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
    ui.label(format!(
        "{} · {} · row {}",
        target.table,
        target.column,
        u64::from(target.parent_ordinal) + 1
    ));
}

fn stored_cell(ui: &mut egui::Ui, value: &Cell) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 26.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let _ = datasheet::cell(ui, value, "Stored value", 1, true);
        },
    );
}
