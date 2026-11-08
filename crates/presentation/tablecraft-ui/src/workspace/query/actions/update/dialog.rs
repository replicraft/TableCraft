//! Compact UPDATE warning adaptation; no native warning capture is retained.
use super::{Phase, Report};
use crate::localization::{message, tr};
use crate::workspace::{Action, EditSaveOutcome, State, diagnostics, save_display};
use egui::{Id, Ui};

const DIALOG_MIN_WIDTH: f32 = 300.0;
const DIALOG_MAX_WIDTH: f32 = 520.0;
const VIEWPORT_MARGIN: f32 = 80.0;
const INNER_MARGIN: i8 = 12;

impl State {
    pub(in crate::workspace) fn query_update_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        self.validate_query_update();
        let can_apply = self.query_update.as_ref().is_some_and(|session| {
            if let Phase::Review { review, .. } = &session.phase {
                self.can_apply_query_update(review.id)
            } else {
                false
            }
        });
        let Some(session) = &mut self.query_update else {
            return;
        };
        let mut close = false;
        let mut apply = None;
        let mut cancel_review = None;
        let frame =
            egui::Frame::popup(&context.style_of(context.theme())).inner_margin(INNER_MARGIN);
        let response = egui::Modal::new(Id::new("query_update_review"))
            .frame(frame)
            .show(context, |ui| {
                ui.set_width(
                    (context.content_rect().width() - VIEWPORT_MARGIN)
                        .clamp(DIALOG_MIN_WIDTH, DIALOG_MAX_WIDTH),
                );
                ui.strong(tr(ui, "query-update-title"));
                ui.separator();
                match &mut session.phase {
                    Phase::Reviewing { request_id, error } => {
                        if let Some(error) = error {
                            report(ui, "query_update_review_error", error);
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr(ui, "query-update-reviewing"));
                            });
                        }
                        ui.separator();
                        if ui.button(tr(ui, "shell-cancel")).clicked() {
                            cancel_review = Some(*request_id);
                            close = true;
                        }
                    }
                    Phase::Review {
                        review,
                        error,
                        focus_no,
                        request_id,
                        ..
                    } => {
                        ui.label(message(
                            ui,
                            "query-update-table",
                            &[("table", review.table.clone())],
                        ));
                        ui.label(message(
                            ui,
                            "query-update-matched",
                            &[("count", review.matched_rows.to_string())],
                        ));
                        ui.label(message(
                            ui,
                            "query-update-changed",
                            &[("count", review.changed_rows.to_string())],
                        ));
                        ui.label(tr(ui, "query-update-confirmation"));
                        ui.small(tr(ui, "query-update-no-undo"));
                        if let Some(error) = error {
                            report(ui, "query_update_apply_error", error);
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    can_apply,
                                    egui::Button::new(tr(ui, "query-update-yes")),
                                )
                                .clicked()
                            {
                                apply = Some(review.id);
                            }
                            let no = ui.button(tr(ui, "query-update-no"));
                            if *focus_no {
                                no.request_focus();
                                *focus_no = false;
                            }
                            close = no.clicked();
                            if close {
                                cancel_review = Some(*request_id);
                            }
                        });
                    }
                    Phase::Saving {
                        save_id,
                        cancel_requested,
                        resolving,
                    } => {
                        save_display::pending(ui, *cancel_requested, *resolving);
                        ui.separator();
                        if !*cancel_requested
                            && ui.button(tr(ui, "editing-request-cancellation")).clicked()
                        {
                            *cancel_requested = true;
                            actions.push(Action::CancelEditSave { save_id: *save_id });
                        }
                    }
                    Phase::Finished {
                        save_id,
                        outcome,
                        report_refused,
                    } => {
                        if *report_refused {
                            ui.label(tr(ui, "objects-memory-limit"));
                        }
                        save_display::outcome(ui, outcome);
                        ui.separator();
                        terminal_buttons(ui, *save_id, outcome, actions);
                    }
                }
            });
        if response.should_close() {
            match &mut session.phase {
                Phase::Reviewing { request_id, .. } => {
                    cancel_review = Some(*request_id);
                    close = true;
                }
                Phase::Review { request_id, .. } => {
                    cancel_review = Some(*request_id);
                    close = true;
                }
                Phase::Saving {
                    save_id,
                    cancel_requested,
                    ..
                } => {
                    if !*cancel_requested {
                        *cancel_requested = true;
                        actions.push(Action::CancelEditSave { save_id: *save_id });
                    }
                }
                Phase::Finished {
                    save_id, outcome, ..
                } => {
                    if !matches!(outcome, EditSaveOutcome::Retained { .. }) {
                        actions.push(Action::AcknowledgeEditSave { save_id: *save_id });
                    }
                }
            }
        }
        if let Some(request_id) = cancel_review {
            actions.push(Action::CancelQueryUpdateReview { request_id });
        }
        if close {
            self.close_query_update();
        } else if let Some(review_id) = apply
            && self.can_apply_query_update(review_id)
        {
            actions.push(Action::ApplyQueryUpdate { review_id });
        }
    }
}

fn terminal_buttons(
    ui: &mut Ui,
    save_id: u64,
    outcome: &EditSaveOutcome,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| match outcome {
        EditSaveOutcome::Retained {
            can_recover,
            can_release,
            ..
        } => {
            if *can_recover && ui.button(tr(ui, "editing-recover")).clicked() {
                actions.push(Action::RecoverEditSave { save_id });
            }
            if *can_release && ui.button(tr(ui, "editing-release")).clicked() {
                actions.push(Action::ReleaseEditSave { save_id });
            }
        }
        _ => {
            if ui.button(tr(ui, "backstage-close")).clicked() {
                actions.push(Action::AcknowledgeEditSave { save_id });
            }
        }
    });
}

fn report(ui: &mut Ui, id: &str, report: &Report) {
    match report {
        Report::Message(message) => diagnostics::show(ui, ui.id().with(id), message),
        Report::Limit => {
            ui.label(tr(ui, "objects-memory-limit"));
        }
    }
}
