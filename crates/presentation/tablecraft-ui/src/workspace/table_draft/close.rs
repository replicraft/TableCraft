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
        let invalid_name = self.object_tabs().iter().any(|tab| {
            self.table_draft_editor(tab)
                .is_some_and(|editor| editor.rename.is_some() && editor.error)
        });
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        let response = Modal::new(egui::Id::new("table_draft_close")).show(context, |ui| {
            ui.heading(tr(ui, "table-draft-close-title"));
            ui.label(tr(ui, "table-draft-close-question"));
            if invalid_name {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    tr(ui, "table-draft-invalid-name"),
                );
            }
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
            // Attempted save consumes an intent on success; the original close intent must be
            // restored unchanged if save preparation refuses.
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
                    if let (Some(revision), Some(record_revision), Some(property_revision)) = (
                        self.table_draft_revision_for(&tab),
                        self.table_draft_record_revision_for(&tab),
                        self.table_draft_property_revision_for(&tab),
                    ) {
                        self.table_draft_discard_approved =
                            // Action::CloseObject needs its own captured identity after this
                            // render/dispatch; keep the original catalog, input or owner
                            // available for the later checks in this operation.
                            Some((tab.clone(), revision, record_revision, property_revision));
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
