//! An explicit keyless-save decision is bound to the original draft witnesses.
use super::{Phase, SaveDialog};
use crate::{Action, localization::tr, workspace::State};
use egui::{Context, Modal};

const KEY_PROMPT_WIDTH: f32 = 430.0;

impl State {
    pub(super) fn table_draft_key_dialog(
        &mut self,
        context: &Context,
        mut save: SaveDialog,
        actions: &mut Vec<Action>,
    ) {
        let available = self.table_save_available && self.save_binding(&save);
        let automatic_available = available && self.generated_table_available();
        let mut automatic = false;
        let mut keyless = false;
        let mut cancel = false;
        let response =
            Modal::new(egui::Id::new("manual_table_primary_choice")).show(context, |ui| {
                ui.set_max_width(KEY_PROMPT_WIDTH);
                ui.heading(tr(ui, "table-draft-no-key-title"));
                ui.label(tr(ui, "table-draft-no-key-question"));
                if let Some(error) = &save.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if !available {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        tr(ui, "table-draft-save-stale"),
                    );
                }
                ui.horizontal(|ui| {
                    let yes = ui.add_enabled(
                        automatic_available,
                        egui::Button::new(tr(ui, "table-draft-property-yes")),
                    );
                    let yes = yes.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
                    if save.focus_name && automatic_available {
                        yes.request_focus();
                    }
                    automatic = yes.clicked();
                    let no = ui.add_enabled(
                        available,
                        egui::Button::new(tr(ui, "table-draft-property-no")),
                    );
                    if save.focus_name {
                        if !automatic_available {
                            no.request_focus();
                        }
                        save.focus_name = false;
                    }
                    keyless = no.clicked();
                    let cancelled = ui.button(tr(ui, "table-draft-cancel"));
                    cancel = cancelled.clicked();
                });
            });
        if cancel || response.should_close() {
            if let Some(editor) = self.table_draft_editor_mut(&save.tab) {
                editor.save_name = save.name;
            }
            self.table_draft_close = save.close;
            return;
        }
        if automatic && automatic_available {
            let editor = self.table_draft_editor_mut(&save.tab).unwrap();
            let Some(record_revision) = editor.record.prepare_generated_primary(&editor.draft)
            else {
                save.error = Some(self.localizer.text("insertion-input-limit"));
                self.table_draft_save = Some(save);
                return;
            };
            match editor.draft.add_incrementing_primary_key() {
                Err(_) => save.error = Some(self.localizer.text("table-draft-no-key-failed")),
                Ok(field) => {
                    editor
                        .record
                        .prepend_generated_primary(field, record_revision);
                    save.revision = editor.draft.revision();
                    save.record_revision = editor.record_revision();
                    save.property_revision = editor.property_revision();
                    save.phase = Phase::Naming;
                    self.issue_table_save(&mut save, actions, false);
                }
            }
        } else if keyless && available {
            save.phase = Phase::Naming;
            self.issue_table_save(&mut save, actions, true);
        }
        self.table_draft_save = Some(save);
    }
}
