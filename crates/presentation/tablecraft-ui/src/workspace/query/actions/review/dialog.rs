//! Compact action warning adaptation; no native warning capture is retained.
use super::{Phase, QueryActionEffect, QueryActionKind, QuerySchemaEffect, Report};
use crate::localization::{message, tr};
use crate::workspace::{Action, EditSaveOutcome, State, diagnostics, save_display};
use egui::{Id, Ui};

const DIALOG_MIN_WIDTH: f32 = 300.0;
const DIALOG_MAX_WIDTH: f32 = 520.0;
const VIEWPORT_MARGIN: f32 = 80.0;
const INNER_MARGIN: i8 = 12;

impl State {
    pub(in crate::workspace) fn query_action_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        self.validate_query_action();
        let can_apply = self.query_action.as_ref().is_some_and(|session| {
            if let Phase::Review { review, .. } = &session.phase {
                self.can_apply_query_action(review.id)
            } else {
                false
            }
        });
        let Some(session) = &mut self.query_action else {
            return;
        };
        let mut close = false;
        let mut apply = None;
        let mut cancel_review = None;
        let frame =
            egui::Frame::popup(&context.style_of(context.theme())).inner_margin(INNER_MARGIN);
        let response = egui::Modal::new(Id::new("query_action_review"))
            .frame(frame)
            .show(context, |ui| {
                ui.set_width(
                    (context.content_rect().width() - VIEWPORT_MARGIN)
                        .clamp(DIALOG_MIN_WIDTH, DIALOG_MAX_WIDTH),
                );
                ui.strong(tr(ui, session.kind.title_key()));
                ui.separator();
                match &mut session.phase {
                    Phase::Reviewing { request_id, error } => {
                        if let Some(error) = error {
                            report(ui, "query_action_review_error", error);
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr(ui, session.kind.reviewing_key()));
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
                            "query-action-table",
                            &[("table", review.table.as_str())],
                        ));
                        if let Some(matched) = review.effect.matched_rows() {
                            ui.label(message(
                                ui,
                                "query-action-matched",
                                &[("count", matched.to_string())],
                            ));
                        }
                        match &review.effect {
                            QueryActionEffect::Update { changed_rows, .. } => {
                                ui.label(message(
                                    ui,
                                    "query-action-update-changed",
                                    &[("count", changed_rows.to_string())],
                                ));
                            }
                            QueryActionEffect::Delete { deleted_rows, .. } => {
                                ui.label(message(
                                    ui,
                                    "query-action-delete-deleted",
                                    &[("count", deleted_rows.to_string())],
                                ));
                            }
                            QueryActionEffect::Insert { inserted_rows } => {
                                ui.label(message(
                                    ui,
                                    "query-action-insert-inserted",
                                    &[("count", inserted_rows.to_string())],
                                ));
                            }
                            QueryActionEffect::Schema(effect) => schema_summary(ui, effect),
                        }
                        let confirmation = match &review.effect {
                            QueryActionEffect::Schema(QuerySchemaEffect::CreateTable {
                                ..
                            }) => "query-action-schema-create-confirmation",
                            QueryActionEffect::Schema(QuerySchemaEffect::DropTable { .. }) => {
                                "query-action-schema-drop-table-confirmation"
                            }
                            QueryActionEffect::Schema(QuerySchemaEffect::DropColumn { .. }) => {
                                "query-action-schema-drop-column-confirmation"
                            }
                            _ => session.kind.confirmation_key(),
                        };
                        ui.label(tr(ui, confirmation));
                        ui.small(tr(ui, "query-action-no-undo"));
                        if let Some(error) = error {
                            report(ui, "query_action_apply_error", error);
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    can_apply,
                                    egui::Button::new(tr(ui, "query-action-yes")),
                                )
                                .clicked()
                            {
                                apply = Some(review.id);
                            }
                            let no = ui.button(tr(ui, "query-action-no"));
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
                        ..
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
            actions.push(Action::CancelQueryActionReview { request_id });
        }
        if close {
            self.close_query_action();
        } else if let Some(review_id) = apply
            && self.can_apply_query_action(review_id)
        {
            actions.push(Action::ApplyQueryAction { review_id });
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

impl QueryActionKind {
    fn title_key(self) -> &'static str {
        match self {
            Self::Update => "query-action-update-title",
            Self::Delete => "query-action-delete-title",
            Self::Insert => "query-action-insert-title",
            Self::Schema => "query-action-schema-title",
        }
    }

    fn reviewing_key(self) -> &'static str {
        match self {
            Self::Update => "query-action-update-reviewing",
            Self::Delete => "query-action-delete-reviewing",
            Self::Insert => "query-action-insert-reviewing",
            Self::Schema => "query-action-schema-reviewing",
        }
    }

    fn confirmation_key(self) -> &'static str {
        match self {
            Self::Update => "query-action-update-confirmation",
            Self::Delete => "query-action-delete-confirmation",
            Self::Insert => "query-action-insert-confirmation",
            Self::Schema => "query-action-schema-confirmation",
        }
    }
}

fn schema_summary(ui: &mut Ui, effect: &QuerySchemaEffect) {
    match effect {
        QuerySchemaEffect::CreateTable { column_count } => {
            ui.label(message(
                ui,
                "query-action-schema-create-fields",
                &[("count", column_count.to_string())],
            ));
        }
        QuerySchemaEffect::DropTable { declared_rows } => {
            ui.label(message(
                ui,
                "query-action-schema-drop-records",
                &[("count", declared_rows.to_string())],
            ));
        }
        QuerySchemaEffect::DropColumn { column, .. } => {
            ui.label(message(
                ui,
                "query-action-schema-drop-field",
                &[("column", column.as_str())],
            ));
        }
    }
}
