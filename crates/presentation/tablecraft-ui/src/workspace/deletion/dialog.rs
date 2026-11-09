//! Confirmation of one runtime-reviewed native row, before consuming its capability.
use super::Dialog;
use crate::datasheet;
use crate::localization::{message, tr};
use crate::workspace::{Action, EditSaveOutcome, State, diagnostics, save_display};
use egui::{Id, Ui};

const MODAL_VERTICAL_RESERVE: f32 = 180.0;
const MIN_BODY_HEIGHT: f32 = 120.0;
const MODAL_INNER_MARGIN: i8 = 12;

impl State {
    pub(in crate::workspace) fn deletion_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let busy = self.busy;
        let synthetic = self.mock_preview;
        let Some(dialog) = &mut self.deletion else {
            return;
        };
        let mut close = false;
        let frame =
            egui::Frame::popup(&context.style_of(context.theme())).inner_margin(MODAL_INNER_MARGIN);
        let response = egui::Modal::new(Id::new("offline_deletion"))
            .frame(frame)
            .show(context, |ui| {
                ui.set_width((context.content_rect().width() - 80.0).clamp(320.0, 640.0));
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                ui.heading(tr(ui, "deletion-title"));
                ui.label(tr(ui, "editing-exclusive-help"));
                if synthetic {
                    ui.small(tr(ui, "deletion-mock-help"));
                }
                ui.separator();
                let body_height =
                    (context.content_rect().height() - MODAL_VERTICAL_RESERVE).max(MIN_BODY_HEIGHT);
                egui::ScrollArea::vertical()
                    .id_salt("delete_body")
                    .auto_shrink([false, true])
                    .min_scrolled_height(body_height)
                    .max_height(body_height)
                    .show(ui, |ui| match dialog {
                        Dialog::Checking { target, error, .. } => {
                            ui.label(message(
                                ui,
                                "deletion-target",
                                &[("table", target.table.as_str())],
                            ));
                            if let Some(error) = error {
                                diagnostics::show(ui, ui.id().with("delete_check_error"), error);
                            } else {
                                ui.spinner();
                                ui.label(tr(ui, "deletion-checking"));
                            }
                        }
                        Dialog::Review { review, error, .. } => {
                            ui.label(message(
                                ui,
                                "deletion-target",
                                &[("table", review.target.table.as_str())],
                            ));
                            ui.label(message(
                                ui,
                                "deletion-record",
                                &[(
                                    "ordinal",
                                    (u64::from(review.target.parent_ordinal) + 1).to_string(),
                                )],
                            ));
                            ui.label(tr(ui, "deletion-confirmation"));
                            ui.label(tr(ui, "deletion-no-undo"));
                            ui.label(message(
                                ui,
                                "deletion-row-count",
                                &[("count", review.deleted_rows.to_string())],
                            ));
                            for (column, cell) in review.columns.iter().zip(&review.row) {
                                ui.separator();
                                ui.label(column.label()).on_hover_text(&column.name);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), 26.0),
                                    egui::Layout::top_down(egui::Align::LEFT),
                                    |ui| {
                                        datasheet::cell(
                                            ui,
                                            cell,
                                            &column.name,
                                            u64::from(review.target.parent_ordinal) + 1,
                                            true,
                                        );
                                    },
                                );
                            }
                            if let Some(error) = error {
                                diagnostics::show(ui, ui.id().with("delete_review_error"), error);
                            }
                        }
                        Dialog::Saving {
                            cancel_requested,
                            resolving,
                            ..
                        } => {
                            save_display::pending(ui, *cancel_requested, *resolving);
                        }
                        Dialog::Finished { outcome, .. } => {
                            save_display::outcome(ui, outcome);
                        }
                    });
                let footer_width = footer_width(ui, dialog);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space((ui.available_width() - footer_width).max(0.0));
                    match dialog {
                        Dialog::Checking { target, .. } => {
                            let id =
                                Id::new(("delete_cancel", target.document_revision, &target.table));
                            close = button(ui, id, "shell-cancel", true);
                        }
                        Dialog::Review {
                            review,
                            focus_cancel,
                            ..
                        } => {
                            if button(
                                ui,
                                Id::new(("delete_apply", review.id)),
                                "deletion-apply",
                                !busy,
                            ) {
                                actions.push(Action::ApplyDelete {
                                    review_id: review.id,
                                });
                            }
                            let cancel = ui
                                .scope_builder(
                                    egui::UiBuilder::new().id(Id::new((
                                        "delete_cancel",
                                        review.target.document_revision,
                                        &review.target.table,
                                    ))),
                                    |ui| ui.button(tr(ui, "shell-cancel")),
                                )
                                .inner;
                            if *focus_cancel {
                                cancel.request_focus();
                                *focus_cancel = false;
                            }
                            close = cancel.clicked();
                        }
                        Dialog::Saving {
                            save_id,
                            cancel_requested,
                            ..
                        } => {
                            if !*cancel_requested
                                && button(
                                    ui,
                                    Id::new(("delete_cancel_save", *save_id)),
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
                                        Id::new(("delete_recover", *save_id)),
                                        "editing-recover",
                                        true,
                                    )
                                {
                                    actions.push(Action::RecoverEditSave { save_id: *save_id });
                                }
                                if *can_release
                                    && button(
                                        ui,
                                        Id::new(("delete_release", *save_id)),
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
                                    Id::new(("delete_acknowledge", *save_id)),
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
            }) = &mut self.deletion
            {
                if !*cancel_requested {
                    *cancel_requested = true;
                    actions.push(Action::CancelEditSave { save_id: *save_id });
                }
            } else {
                close = true;
            }
        }
        if close && !self.delete_save_pending() {
            match self.deletion.take() {
                Some(Dialog::Checking { request_id, .. } | Dialog::Review { request_id, .. }) => {
                    actions.push(Action::CloseDelete { request_id });
                }
                Some(Dialog::Finished { save_id, .. }) => {
                    actions.push(Action::AcknowledgeEditSave { save_id })
                }
                _ => {}
            }
        }
    }
}

fn footer_width(ui: &Ui, dialog: &Dialog) -> f32 {
    let keys = match dialog {
        Dialog::Checking { .. } => vec!["shell-cancel"],
        Dialog::Review { .. } => vec!["deletion-apply", "shell-cancel"],
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
            let text = ui
                .painter()
                // The layout API consumes this FontId; keep the original style or
                // loop-local descriptor available for the next measurement.
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
