use super::{NEXT_SAVE_REQUEST, Ordering, Phase, SaveDialog, State, TableSaveRequest};
use crate::Action;
use crate::localization::tr;
use egui::{Context, Id, Modal, TextEdit};
use std::borrow::Cow;
use tablecraft_builders::table::{MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

const NAME_WIDTH: f32 = 280.0;
const MAX_STATUS_HEIGHT: f32 = 160.0;

impl State {
    pub(in crate::workspace) fn table_draft_save_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(mut save) = self.table_draft_save.take() else {
            return;
        };
        if save.phase == Phase::ChoosingKey {
            self.table_draft_key_dialog(context, save, actions);
            return;
        }
        let naming = save.phase == Phase::Naming;
        let mut confirm = false;
        let mut cancel = false;
        let mut recover = false;
        let mut release = false;
        let mut acknowledge = false;
        let response = Modal::new(Id::new("initial_table_save_name")).show(context, |ui| {
            ui.set_max_width(NAME_WIDTH);
            ui.heading(tr(ui, "table-draft-save-as"));
            let label = ui.label(tr(ui, "table-draft-table-name"));
            let input_id = ui.make_persistent_id("table_name");
            // Consume the name field's Enter before single-line TextEdit can
            // surrender its focus. Another modal control never borrows this chord.
            let confirm_from_name = naming
                && ui.memory(|memory| memory.has_focus(input_id))
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            let mut edited_name = Cow::Borrowed(save.name.as_str());
            let rejected = crate::text_input::reject_events_with_limits(
                ui,
                input_id,
                edited_name.as_ref(),
                MAX_DRAFT_NAME_BYTES,
                MAX_DRAFT_NAME_UNITS,
            );
            let name = ui
                .add_enabled(
                    naming,
                    TextEdit::singleline(&mut edited_name)
                        .id(input_id)
                        .desired_width(NAME_WIDTH),
                )
                .labelled_by(label.id);
            if rejected
                || edited_name.len() > MAX_DRAFT_NAME_BYTES
                || edited_name
                    .encode_utf16()
                    .take(MAX_DRAFT_NAME_UNITS + 1)
                    .count()
                    > MAX_DRAFT_NAME_UNITS
            {
                save.error = Some(tr(ui, "table-draft-invalid-table-name"));
            } else if let Cow::Owned(name) = edited_name {
                save.name = name;
            }
            if naming && save.focus_name {
                name.request_focus();
                save.focus_name = false;
            }
            if let Some(error) = &save.error {
                egui::ScrollArea::vertical()
                    .id_salt("table_save_status")
                    .max_height(MAX_STATUS_HEIGHT)
                    .show(ui, |ui| {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    });
            }
            if !naming {
                ui.horizontal(|ui| {
                    if save.phase != Phase::Retained {
                        ui.spinner();
                    }
                    ui.add(
                        egui::Label::new(tr(
                            ui,
                            if save.phase == Phase::Retained {
                                "table-draft-save-retained"
                            } else if save.cancel_requested {
                                "table-draft-save-cancelling"
                            } else {
                                "table-draft-save-working"
                            },
                        ))
                        .wrap(),
                    );
                });
            }
            if save.phase == Phase::Retained {
                ui.horizontal_wrapped(|ui| {
                    if save.can_recover {
                        recover = ui
                            .add_enabled(
                                !save.recovery_requested,
                                egui::Button::new(tr(ui, "editing-recover")),
                            )
                            .clicked();
                    }
                    if save.can_release {
                        release = ui
                            .add_enabled(
                                !save.recovery_requested,
                                egui::Button::new(tr(ui, "editing-release")),
                            )
                            .clicked();
                    }
                    if save.can_acknowledge {
                        acknowledge = ui
                            .add_enabled(
                                !save.recovery_requested,
                                egui::Button::new(tr(ui, "backstage-close")),
                            )
                            .clicked();
                    }
                });
                if !save.can_recover && !save.can_release && !save.can_acknowledge {
                    ui.label(tr(ui, "editing-diagnosis-required"));
                }
            } else {
                ui.horizontal(|ui| {
                    confirm = ui
                        .add_enabled(
                            naming && self.table_save_available,
                            egui::Button::new(tr(ui, "table-draft-ok")),
                        )
                        .clicked();
                    cancel = ui
                        .add_enabled(
                            !save.cancel_requested,
                            egui::Button::new(tr(ui, "table-draft-cancel")),
                        )
                        .clicked();
                });
            }
            if confirm_from_name {
                confirm = self.table_save_available;
            }
        });
        cancel |= response.should_close() && save.phase != Phase::Retained;
        if recover || release || acknowledge {
            if !save.recovery_requested
                && let Some(id) = save.id
            {
                save.recovery_requested = true;
                actions.push(if recover {
                    Action::RecoverTableDraftSave { id }
                } else if release {
                    Action::ReleaseTableDraftSave { id }
                } else {
                    Action::AcknowledgeTableDraftSave { id }
                });
            }
        } else if cancel {
            if naming {
                if let Some(editor) = self.table_draft_editor_mut(&save.tab) {
                    editor.save_name = save.name;
                }
                self.table_draft_close = save.close;
                return;
            }
            if !save.cancel_requested
                && let Some(id) = save.id
            {
                save.cancel_requested = true;
                actions.push(Action::CancelTableDraftSave { id });
            }
        } else if confirm {
            self.issue_table_save(&mut save, actions, false);
        }
        self.table_draft_save = Some(save);
    }

    pub(super) fn issue_table_save(
        &mut self,
        save: &mut SaveDialog,
        actions: &mut Vec<Action>,
        allow_keyless: bool,
    ) {
        if save.phase != Phase::Naming || !self.table_save_available || !self.save_binding(save) {
            save.error = Some(self.localizer.text("table-draft-save-stale"));
            return;
        }
        let Some(editor) = self.table_draft_editor(&save.tab) else {
            save.error = Some(self.localizer.text("table-draft-save-stale"));
            return;
        };
        let snapshot = match editor.draft.snapshot(&save.name) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                save.error = Some(self.localizer.text("table-draft-invalid-table-name"));
                return;
            }
        };
        if editor.draft.is_design()
            && !snapshot.fields().iter().any(|field| field.is_primary())
            && !allow_keyless
        {
            save.phase = Phase::ChoosingKey;
            save.focus_name = true;
            save.error = None;
            return;
        }
        let Ok(previous) =
            NEXT_SAVE_REQUEST
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            save.error = Some(self.localizer.text("table-draft-save-failed"));
            return;
        };
        let id = previous + 1;
        if let Some(editor) = self.table_draft_editor_mut(&save.tab) {
            // NO_CLONE: the retained command or exact-owner witness needs its own captured
            // identity after this render/dispatch; keep the original catalog, input
            // or owner available for the later checks in this operation.
            editor.save_name = save.name.clone();
        }
        save.phase = Phase::Issued;
        save.id = Some(id);
        save.error = None;
        actions.push(Action::SaveTableDraft {
            request: TableSaveRequest {
                id,
                // NO_CLONE: TableSaveRequest needs its own captured identity after this
                // render/dispatch; keep the original catalog, input or owner
                // available for the later checks in this operation.
                tab: save.tab.clone(),
                snapshot,
                record_revision: save.record_revision,
                property_revision: save.property_revision,
            },
        });
    }
}

#[cfg(test)]
mod tests;
