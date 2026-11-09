use super::{body_height, size};
use crate::{
    Action, State,
    localization::{message, tr},
    workspace::{
        diagnostics,
        field_drop::{
            FieldDropConfirmationDecision as Decision,
            model::{Observation, Phase},
        },
    },
};
use egui::{Context, Id, Modal};

impl State {
    pub(in crate::workspace) fn field_drop_confirmation_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(draft) = self.field_drop.draft.as_ref() else {
            return;
        };
        if draft.phase != Phase::Confirming {
            return;
        }
        let can_yes = self.can_confirm_field_drop(&draft.target, draft.revision);
        let can_retry = !self.busy
            && self.field_drop_context(&draft.target)
            && !matches!(draft.observation, Observation::Pending { .. });
        let mut choice = None;
        let mut retry = false;
        // The frame exclusively borrows the actual retained owner. No target
        // clone, deferred retargeting or callback into application State occurs.
        let draft = self.field_drop.draft.as_mut().unwrap();
        let response = Modal::new(Id::new("field_drop_confirmation")).show(context, |ui| {
            size(ui, context);
            ui.heading(tr(ui, "field-drop-title"));
            egui::ScrollArea::vertical()
                .max_height(body_height(context))
                .show(ui, |ui| {
                    ui.label(message(
                        ui,
                        "field-drop-confirm",
                        &[("column", draft.target.column.as_str().into())],
                    ));
                    ui.label(tr(ui, "field-drop-data-loss"));
                    ui.separator();
                    match &draft.observation {
                        Observation::Pending { .. } => {
                            ui.spinner();
                            ui.label(tr(ui, "field-drop-inspecting"));
                        }
                        Observation::Eligible => {}
                        Observation::Unsupported => {
                            ui.label(tr(ui, "field-drop-source-unavailable"));
                        }
                        Observation::Failed { message } if message.is_empty() => {
                            ui.label(tr(ui, "objects-memory-limit"));
                        }
                        Observation::Failed { message } => {
                            diagnostics::show(ui, ui.id().with("drop_source_error"), message);
                        }
                        Observation::Interrupted => {
                            ui.label(tr(ui, "field-drop-inspection-interrupted"));
                        }
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(can_yes, egui::Button::new(tr(ui, "field-drop-yes")))
                    .clicked()
                {
                    choice = Some(Decision::Yes);
                }
                let no = ui.button(tr(ui, "field-drop-no"));
                if draft.focus_no {
                    no.request_focus();
                }
                if no.clicked() {
                    choice = Some(Decision::No);
                }
                if can_retry && ui.button(tr(ui, "field-drop-retry-inspection")).clicked() {
                    retry = true;
                }
            });
        });
        draft.focus_no = false;
        if response.should_close() {
            choice = Some(Decision::No);
        }
        if let Some(choice) = choice {
            if (choice == Decision::No || can_yes)
                && let Some(result) = self.field_drop.choose_confirmation(choice)
                && let Some(request_id) = result.inspection_request_id
            {
                actions.push(Action::CancelFieldDropInspection { request_id });
            }
        } else if retry {
            // Exact owner remains exclusively borrowed for this whole frame;
            // the post-widget collector captures its actual metadata afresh.
            self.field_drop.draft.as_mut().unwrap().observation =
                Observation::Pending { request_id: None };
        }
    }
}
