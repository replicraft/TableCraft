//! Existing query Save/Discard/Cancel presentation, bound to a real Design revision.
use super::QueryDesignCloseDecision as Decision;
use crate::localization::{message, tr};
use crate::{Action, State};
use egui::{Context, Id, Modal};
use std::borrow::Cow;

impl State {
    pub(in crate::workspace) fn query_design_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if !self.query_design_close_open() {
            return;
        }
        let Some(intent) = &self.query_design_close.intent else {
            return;
        };
        let current = self.modified_query_design_for(&intent.request.tab)
            == Some((intent.text_revision, intent.draft_revision));
        let title = match &intent.request.tab.target.kind {
            crate::ObjectKind::SavedQuery { name, .. } => Cow::Borrowed(name.as_str()),
            crate::ObjectKind::Sql => Cow::Owned(self.localizer.text("shell-query")),
            _ => return,
        };
        let mut decision = None;
        let response = Modal::new(Id::new("query_design_close")).show(context, |ui| {
            ui.set_max_width(420.0);
            ui.heading(tr(ui, "saved-query-close-title"));
            ui.label(message(
                ui,
                "saved-query-close-question",
                &[("name", title.as_ref())],
            ));
            if !current {
                ui.label(tr(ui, "saved-query-close-stale"));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(current, egui::Button::new(tr(ui, "saved-query-close-save")))
                    .clicked()
                {
                    decision = Some(Decision::Save);
                }
                if ui
                    .add_enabled(
                        current,
                        egui::Button::new(tr(ui, "saved-query-close-discard")),
                    )
                    .clicked()
                {
                    decision = Some(Decision::Discard);
                }
                if ui.button(tr(ui, "saved-query-close-cancel")).clicked() {
                    decision = Some(Decision::Cancel);
                }
            });
        });
        if response.should_close() {
            decision = Some(Decision::Cancel);
        }
        if let Some(decision) = decision
            && let Some(result) = self.choose_query_design_close(decision)
        {
            actions.push(Action::QueryDesignClose { result });
        }
    }
}
