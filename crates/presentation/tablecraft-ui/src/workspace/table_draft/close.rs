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
        let can_save = self.can_save_table_draft_close();
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        let response = Modal::new(egui::Id::new("table_draft_close")).show(context, |ui| {
            ui.heading(tr(ui, "table-draft-close-title"));
            ui.label(tr(ui, "table-draft-close-question"));
            if !can_save {
                ui.label(tr(ui, "table-draft-save-unavailable"));
            }
            ui.horizontal(|ui| {
                save = ui
                    .add_enabled(can_save, egui::Button::new(tr(ui, "table-draft-save")))
                    .on_disabled_hover_text(tr(ui, "table-draft-save-unavailable"))
                    .clicked();
                discard = ui.button(tr(ui, "table-draft-discard")).clicked();
                cancel = ui.button(tr(ui, "table-draft-cancel")).clicked();
            });
        });
        cancel |= response.should_close();
        if !save && !discard && !cancel {
            return;
        }
        let Some(intent) = self.table_draft_close.take() else {
            return;
        };
        if save {
            if !self.start_table_draft_save(Some(intent.clone())) {
                self.table_draft_close = Some(intent);
            }
        } else if cancel {
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
