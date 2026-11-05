//! Explicit field intent and runtime review; this renderer never supplies defaults.
use super::{Choice, Dialog, FieldInput, entries_within_limits, inputs};
use crate::localization::{message, tr};
use crate::workspace::{Action, EditSaveOutcome, State, diagnostics};
use egui::{Id, Ui};

const MODAL_VERTICAL_RESERVE: f32 = 180.0;
const MIN_BODY_HEIGHT: f32 = 120.0;
const MODAL_INNER_MARGIN: i8 = 12;

impl State {
    pub(in crate::workspace) fn insertion_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let busy = self.busy;
        let synthetic = self.mock_preview;
        let Some(dialog) = &mut self.insertion else {
            return;
        };
        let mut close = false;
        let frame =
            egui::Frame::popup(&context.style_of(context.theme())).inner_margin(MODAL_INNER_MARGIN);
        let response = egui::Modal::new(Id::new("offline_insertion"))
            .frame(frame)
            .show(context, |ui| {
                ui.set_width((context.content_rect().width() - 80.0).clamp(320.0, 640.0));
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                ui.heading(tr(ui, "insertion-title"));
                ui.label(tr(ui, "editing-exclusive-help"));
                if synthetic {
                    ui.small(tr(ui, "insertion-mock-help"));
                }
                ui.separator();
                let body_height =
                    (context.content_rect().height() - MODAL_VERTICAL_RESERVE).max(MIN_BODY_HEIGHT);
                egui::ScrollArea::vertical()
                    .id_salt("insert_body")
                    .auto_shrink([false, true])
                    .min_scrolled_height(body_height)
                    .max_height(body_height)
                    .show(ui, |ui| match dialog {
                        Dialog::Checking { target, error, .. } => {
                            ui.label(message(
                                ui,
                                "insertion-target",
                                &[("table", target.table.clone())],
                            ));
                            if let Some(error) = error {
                                diagnostics::show(ui, ui.id().with("insert_check_error"), error);
                            } else {
                                ui.spinner();
                                ui.label(tr(ui, "insertion-checking"));
                            }
                        }
                        Dialog::Draft {
                            draft,
                            entries,
                            pending,
                            review,
                            error,
                            ..
                        } => {
                            ui.label(message(
                                ui,
                                "insertion-target",
                                &[("table", draft.target.table.clone())],
                            ));
                            ui.label(tr(ui, "insertion-intent-help"));
                            let mut changed = false;
                            for (position, (field, entry)) in
                                draft.fields.iter().zip(entries.iter_mut()).enumerate()
                            {
                                ui.push_id((draft.id, position), |ui| {
                                    ui.separator();
                                    let label = ui.label(&field.name);
                                    ui.small(&field.input_kind);
                                    changed |= field_input(ui, entry, field.nullable, label.id);
                                });
                            }
                            if changed {
                                *pending = None;
                                *review = None;
                                *error = None;
                            }
                            if let Some(error) = error {
                                diagnostics::show(ui, ui.id().with("insert_review_error"), error);
                            }
                            if pending.is_some() {
                                ui.spinner();
                                ui.label(tr(ui, "insertion-validating"));
                            }
                            if !entries_within_limits(entries) {
                                ui.label(tr(ui, "insertion-input-limit"));
                            }
                            if let Some(review) = review {
                                ui.separator();
                                ui.label(tr(ui, "insertion-reviewed"));
                                ui.label(message(
                                    ui,
                                    "insertion-row-count",
                                    &[("count", review.inserted_rows.to_string())],
                                ));
                            }
                        }
                        Dialog::Saving {
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
                        Dialog::Finished { outcome, .. } => match outcome {
                            EditSaveOutcome::Retained {
                                message,
                                can_recover,
                                can_release,
                            } => {
                                diagnostics::show(ui, ui.id().with("insert_save_report"), message);
                                ui.label(tr(ui, "editing-recovery-owned"));
                                if !*can_recover && !*can_release {
                                    ui.label(tr(ui, "editing-diagnosis-required"));
                                }
                            }
                            EditSaveOutcome::Saved(message) => {
                                terminal(ui, "editing-outcome-saved", message)
                            }
                            EditSaveOutcome::Cancelled(message) => {
                                terminal(ui, "editing-outcome-cancelled", message)
                            }
                            EditSaveOutcome::Failed(message) => {
                                terminal(ui, "editing-outcome-failed", message)
                            }
                        },
                    });
                let footer_width = footer_width(ui, dialog);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space((ui.available_width() - footer_width).max(0.0));
                    match dialog {
                        Dialog::Checking { target, .. } => {
                            let id =
                                Id::new(("insert_cancel", target.document_revision, &target.table));
                            close = button(ui, id, "shell-cancel", true);
                        }
                        Dialog::Draft {
                            draft,
                            entries,
                            pending,
                            review,
                            ..
                        } => {
                            if let Some(review) = review {
                                let id = Id::new(("insert_apply", review.id));
                                if button(ui, id, "insertion-apply", !busy) {
                                    actions.push(Action::ApplyInsert {
                                        review_id: review.id,
                                    });
                                }
                            }
                            if button(
                                ui,
                                Id::new(("insert_review", draft.id)),
                                "insertion-review",
                                pending.is_none() && !busy && entries_within_limits(entries),
                            ) {
                                actions.push(Action::ReviewInsert {
                                    draft_id: draft.id,
                                    inputs: inputs(entries),
                                });
                            }
                            let id = Id::new((
                                "insert_cancel",
                                draft.target.document_revision,
                                &draft.target.table,
                            ));
                            close = button(ui, id, "shell-cancel", true);
                        }
                        Dialog::Saving {
                            save_id,
                            cancel_requested,
                            ..
                        } => {
                            if !*cancel_requested
                                && button(
                                    ui,
                                    Id::new(("insert_cancel_save", *save_id)),
                                    "editing-request-cancellation",
                                    true,
                                )
                            {
                                *cancel_requested = true;
                                actions.push(Action::CancelEditSave { save_id: *save_id });
                            }
                        }
                        Dialog::Finished { save_id, outcome } => match outcome {
                            EditSaveOutcome::Retained {
                                can_recover,
                                can_release,
                                ..
                            } => {
                                if *can_recover
                                    && button(
                                        ui,
                                        Id::new(("insert_recover", *save_id)),
                                        "editing-recover",
                                        true,
                                    )
                                {
                                    actions.push(Action::RecoverEditSave { save_id: *save_id });
                                }
                                if *can_release
                                    && button(
                                        ui,
                                        Id::new(("insert_release", *save_id)),
                                        "editing-release",
                                        true,
                                    )
                                {
                                    actions.push(Action::ReleaseEditSave { save_id: *save_id });
                                }
                            }
                            _ => {
                                close = button(
                                    ui,
                                    Id::new(("insert_acknowledge", *save_id)),
                                    "backstage-close",
                                    true,
                                )
                            }
                        },
                    }
                });
            });
        if response.should_close() {
            if let Some(Dialog::Saving {
                save_id,
                cancel_requested,
                ..
            }) = &mut self.insertion
            {
                if !*cancel_requested {
                    *cancel_requested = true;
                    actions.push(Action::CancelEditSave { save_id: *save_id });
                }
            } else {
                close = true;
            }
        }
        if close && !self.insert_save_pending() {
            match self.insertion.take() {
                Some(Dialog::Checking { request_id, .. }) => actions.push(Action::CloseInsert {
                    request_id,
                    draft_id: None,
                }),
                Some(Dialog::Draft {
                    request_id, draft, ..
                }) => actions.push(Action::CloseInsert {
                    request_id,
                    draft_id: Some(draft.id),
                }),
                Some(Dialog::Finished { save_id, .. }) => {
                    actions.push(Action::AcknowledgeEditSave { save_id })
                }
                _ => {}
            }
        }
    }
}

fn field_input(ui: &mut Ui, entry: &mut FieldInput, nullable: bool, label: Id) -> bool {
    let mut changed = false;
    let previous = entry.choice;
    ui.horizontal_wrapped(|ui| {
        changed |= ui
            .radio_value(
                &mut entry.choice,
                Choice::Omitted,
                tr(ui, "insertion-omitted"),
            )
            .changed();
        changed |= ui
            .add_enabled_ui(nullable, |ui| {
                ui.radio_value(&mut entry.choice, Choice::Null, tr(ui, "insertion-null"))
            })
            .inner
            .changed();
        changed |= ui
            .radio_value(&mut entry.choice, Choice::Value, tr(ui, "insertion-value"))
            .changed();
    });
    let value = ui
        .add_enabled(
            entry.choice == Choice::Value,
            egui::TextEdit::multiline(&mut entry.text)
                .desired_rows(2)
                .desired_width(ui.available_width()),
        )
        .labelled_by(label);
    crate::theme::input_border(ui, &value);
    let chose_value = previous != Choice::Value && entry.choice == Choice::Value;
    if chose_value {
        value.request_focus();
    }
    if value.gained_focus() || chose_value {
        value.scroll_to_me(None);
    }
    changed | value.changed()
}

fn footer_width(ui: &Ui, dialog: &Dialog) -> f32 {
    let keys = match dialog {
        Dialog::Checking { .. } => vec!["shell-cancel"],
        Dialog::Draft { review, .. } => {
            let mut keys = Vec::new();
            if review.is_some() {
                keys.push("insertion-apply");
            }
            keys.extend(["insertion-review", "shell-cancel"]);
            keys
        }
        Dialog::Saving {
            cancel_requested, ..
        } => {
            if *cancel_requested {
                Vec::new()
            } else {
                vec!["editing-request-cancellation"]
            }
        }
        Dialog::Finished {
            outcome:
                EditSaveOutcome::Retained {
                    can_recover,
                    can_release,
                    ..
                },
            ..
        } => {
            let mut keys = Vec::new();
            if *can_recover {
                keys.push("editing-recover");
            }
            if *can_release {
                keys.push("editing-release");
            }
            keys
        }
        Dialog::Finished { .. } => vec!["backstage-close"],
    };
    let font = egui::TextStyle::Button.resolve(ui.style());
    let padding = ui.spacing().button_padding.x * 2.0;
    let widths: f32 = keys
        .iter()
        .map(|key| {
            let text =
                ui.painter()
                    .layout_no_wrap(tr(ui, key), font.clone(), ui.visuals().text_color());
            text.size().x + padding
        })
        .sum();
    widths + ui.spacing().item_spacing.x * keys.len().saturating_sub(1) as f32
}

fn button(ui: &mut Ui, id: Id, key: &str, enabled: bool) -> bool {
    ui.scope_builder(egui::UiBuilder::new().id(id), |ui| {
        ui.add_enabled(enabled, egui::Button::new(tr(ui, key)))
            .clicked()
    })
    .inner
}

fn terminal(ui: &mut Ui, heading: &str, report: &str) {
    ui.heading(tr(ui, heading));
    diagnostics::show(ui, ui.id().with("insert_terminal_report"), report);
}
