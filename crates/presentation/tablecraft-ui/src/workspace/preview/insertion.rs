//! Synthetic row-insertion responses; no source is opened or persisted.
use super::Preview;
use crate::workspace::insertion::Dialog;
use crate::{
    Action, EditSaveOutcome, InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget,
};

const MOCK_FIELD_COUNT: usize = 3;

impl Preview {
    pub(super) fn prepare_insert_scene(&mut self, reviewed: bool) {
        let target = self
            .state
            .current_insert_target()
            .expect("synthetic native table");
        self.mock_begin_insert(target);
        if reviewed {
            let (draft_id, proposed) = match &self.state.insertion {
                Some(Dialog::Draft { draft, .. }) => (
                    draft.id,
                    draft.fields.iter().map(|_| InsertInput::Omitted).collect(),
                ),
                _ => unreachable!("synthetic insertion draft was published"),
            };
            self.mock_review_insert(draft_id, proposed);
        }
    }

    pub(super) fn mock_begin_insert(&mut self, target: InsertTarget) {
        if !self.state.can_insert_row(&target) {
            return;
        }
        let fields = self
            .state
            .data
            .as_ref()
            .expect("admitted table")
            .columns
            .iter()
            .take(MOCK_FIELD_COUNT)
            .map(|column| InsertField {
                name: column.name.clone(),
                input_kind: self.state.localizer.text("insertion-mock-kind"),
                nullable: true,
            })
            .collect();
        let request = self.request();
        self.state.begin_insert(target.clone(), request);
        self.state.set_insert_draft(
            request,
            InsertDraft {
                id: request,
                target,
                fields,
            },
        );
    }

    pub(super) fn mock_review_insert(&mut self, draft_id: u64, proposed: Vec<InsertInput>) {
        if !self.state.can_review_insert(draft_id, &proposed) {
            return;
        }
        let target = match &self.state.insertion {
            Some(Dialog::Draft { draft, .. }) => draft.target.clone(),
            _ => return,
        };
        let request = self.request();
        self.state
            .begin_insert_review(draft_id, proposed.clone(), request);
        self.state.set_insert_review(
            request,
            InsertReview {
                id: request,
                draft_id,
                target,
                inputs: proposed,
                inserted_rows: 1,
            },
        );
    }

    pub(super) fn mock_apply_insert(&mut self, review_id: u64) {
        if !self.state.can_apply_insert(review_id) {
            return;
        }
        let save_id = self.request();
        self.state.begin_insert_save(review_id, save_id);
        self.state.set_insert_save_outcome(
            save_id,
            EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
        );
    }

    pub(super) fn mock_close_insert(&mut self, request_id: u64, draft_id: Option<u64>) {
        let matches = match &self.state.insertion {
            Some(Dialog::Checking {
                request_id: current,
                ..
            }) => *current == request_id && draft_id.is_none(),
            Some(Dialog::Draft {
                request_id: current,
                draft,
                ..
            }) => *current == request_id && draft_id == Some(draft.id),
            _ => false,
        };
        if matches {
            self.state.close_insert();
        }
    }

    pub(super) fn insertion_save_action(&mut self, action: &Action) -> bool {
        let Some(dialog) = &mut self.state.insertion else {
            return false;
        };
        match (action, dialog) {
            (
                Action::CancelEditSave { save_id },
                Dialog::Saving {
                    save_id: current,
                    cancel_requested,
                    ..
                },
            ) => {
                if save_id == current {
                    *cancel_requested = true;
                }
                true
            }
            (
                Action::Cancel,
                Dialog::Saving {
                    cancel_requested, ..
                },
            ) => {
                *cancel_requested = true;
                true
            }
            (
                Action::AcknowledgeEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome,
                },
            ) => {
                if save_id == current && !matches!(outcome, EditSaveOutcome::Retained { .. }) {
                    self.state.close_insert();
                }
                true
            }
            (
                Action::RecoverEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome: EditSaveOutcome::Retained { can_recover, .. },
                },
            ) if save_id == current && *can_recover => {
                let original = *save_id;
                let operation = self.request();
                self.state.begin_insert_resolution(original, operation);
                self.state.set_insert_save_outcome(
                    operation,
                    EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-resolved")),
                );
                true
            }
            (
                Action::ReleaseEditSave { save_id },
                Dialog::Finished {
                    save_id: current,
                    outcome: EditSaveOutcome::Retained { can_release, .. },
                },
            ) if save_id == current && *can_release => {
                let original = *save_id;
                let operation = self.request();
                self.state.begin_insert_resolution(original, operation);
                self.state.set_insert_save_outcome(
                    operation,
                    EditSaveOutcome::Failed(self.state.localizer.text("settings-mock-resolved")),
                );
                true
            }
            _ => false,
        }
    }
}
