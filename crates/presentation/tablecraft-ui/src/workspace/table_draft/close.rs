use super::{Action, CloseIntent, State};
use crate::localization::tr;
use egui::{Context, Modal};

impl State {
    pub(in crate::workspace) fn table_draft_close_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if self.table_draft_close.is_none() {
            return;
        }
        let mut discard = false;
        let mut cancel = false;
        let response = Modal::new(egui::Id::new("table_draft_close")).show(context, |ui| {
            ui.heading(tr(ui, "table-draft-close-title"));
            ui.label(tr(ui, "table-draft-close-question"));
            ui.label(tr(ui, "table-draft-save-unavailable"));
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new(tr(ui, "table-draft-save")))
                    .on_disabled_hover_text(tr(ui, "table-draft-save-unavailable"));
                discard = ui.button(tr(ui, "table-draft-discard")).clicked();
                cancel = ui.button(tr(ui, "table-draft-cancel")).clicked();
            });
        });
        cancel |= response.should_close();
        if !discard && !cancel {
            return;
        }
        let Some(intent) = self.table_draft_close.take() else {
            return;
        };
        if cancel {
            if matches!(intent, CloseIntent::All) {
                actions.push(Action::TableDraftDiscardCancelled);
            }
        } else {
            match intent {
                CloseIntent::Tab(tab) => {
                    if let Some(revision) = self.table_draft_revision_for(&tab) {
                        self.table_draft_discard_approved = Some((tab.clone(), revision));
                        actions.push(Action::CloseObject { tab });
                    }
                }
                CloseIntent::All => {
                    self.discard_all_table_drafts();
                    actions.push(Action::TableDraftsDiscarded);
                }
            }
        }
    }
}
