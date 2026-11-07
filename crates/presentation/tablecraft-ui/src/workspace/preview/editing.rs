//! Synthetic snapshots of the existing reviewed-edit lifecycle, without save resources.
use super::{Preview, Scene};
use crate::workspace::editing::EditDialog;
use crate::{EditInput, EditSaveOutcome, SaveCleanupNotice};

impl Preview {
    pub(super) fn show_editing_scene(&mut self, scene: Scene) {
        match scene {
            Scene::EditChecking => {
                let data = self.state.data.as_ref().expect("synthetic table page");
                let column = data
                    .column_index("Balance")
                    .expect("synthetic Balance column");
                let target = data
                    .edit_target(0, column)
                    .expect("synthetic scalar edit target");
                let request = self.request();
                self.state.begin_edit(target, request);
            }
            Scene::EditValidating | Scene::EditRejected => {
                self.open_edit();
                let request = self.pending_mock_review();
                if scene == Scene::EditRejected {
                    self.state
                        .fail_edit(request, self.state.localizer.text("editing-mock-rejected"));
                }
            }
            Scene::SaveApplying
            | Scene::SaveCancelling
            | Scene::SaveResolving
            | Scene::SaveSaved
            | Scene::SaveSavedRetainedJournal
            | Scene::SaveCancelled
            | Scene::SaveFailed
            | Scene::RecoveryDiagnosis => {
                let save = self.begin_mock_save();
                match scene {
                    Scene::SaveApplying => {}
                    Scene::SaveCancelling => {
                        // This is the existing UI-local request flag, not an outcome.
                        let Some(EditDialog::Saving {
                            cancel_requested, ..
                        }) = &mut self.state.edit
                        else {
                            unreachable!("synthetic save has started");
                        };
                        *cancel_requested = true;
                    }
                    Scene::SaveResolving => {
                        self.state.set_edit_save_outcome(
                            save,
                            EditSaveOutcome::Retained {
                                message: self.state.localizer.text("settings-mock-retained"),
                                can_recover: true,
                                can_release: true,
                            },
                        );
                        let operation = self.request();
                        self.state.begin_edit_resolution(save, operation);
                    }
                    Scene::SaveSaved => self.state.set_edit_save_outcome(
                        save,
                        EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
                    ),
                    Scene::SaveSavedRetainedJournal => {
                        let mut report = self.state.localizer.text("editing-save-completed");
                        report.push(' ');
                        report.push_str(
                            &self
                                .state
                                .localizer
                                .text("editing-save-retained-private-journal"),
                        );
                        self.state
                            .set_edit_save_outcome(save, EditSaveOutcome::Saved(report));
                        let revision = self
                            .state
                            .database
                            .as_ref()
                            .expect("synthetic database")
                            .document_revision;
                        self.state.set_save_cleanup(
                            revision,
                            Some(SaveCleanupNotice::RetainedPrivateJournal),
                        );
                    }
                    Scene::SaveCancelled => self.state.set_edit_save_outcome(
                        save,
                        EditSaveOutcome::Cancelled(
                            self.state.localizer.text("settings-mock-save-cancelled"),
                        ),
                    ),
                    Scene::SaveFailed => self.state.set_edit_save_outcome(
                        save,
                        EditSaveOutcome::Failed(self.state.localizer.text("editing-mock-failed")),
                    ),
                    Scene::RecoveryDiagnosis => self.state.set_edit_save_outcome(
                        save,
                        EditSaveOutcome::Retained {
                            message: self.state.localizer.text("editing-mock-diagnosis"),
                            can_recover: false,
                            can_release: false,
                        },
                    ),
                    _ => unreachable!("matched only synthetic save scenes"),
                }
            }
            _ => unreachable!("editing scene dispatch is explicit"),
        }
    }

    fn pending_mock_review(&mut self) -> u64 {
        let input = replacement();
        self.preview_review(input.clone())
            .expect("synthetic review fixture is valid");
        let Some(EditDialog::Draft { draft, .. }) = &self.state.edit else {
            unreachable!("synthetic edit draft is present");
        };
        let draft_id = draft.id;
        let request = self.request();
        self.state.begin_edit_review(draft_id, input, request);
        request
    }

    fn begin_mock_save(&mut self) -> u64 {
        self.open_edit();
        let review = self
            .preview_review(replacement())
            .expect("synthetic review fixture is valid");
        let save = self.request();
        self.state.begin_edit_save(review, save);
        save
    }
}

fn replacement() -> EditInput {
    EditInput {
        text: "1495.50".into(),
        is_null: false,
    }
}

#[cfg(test)]
mod tests;
