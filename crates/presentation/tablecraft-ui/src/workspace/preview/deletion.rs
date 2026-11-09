//! Invented reviewed rows; no database mutation occurs in this preview host.
use super::{Preview, actions::copy_cell};
use crate::workspace::deletion::Dialog;
use crate::{Action, DeleteReview, DeleteTarget, EditSaveOutcome};

impl Preview {
    pub(super) fn prepare_delete_scene(&mut self, reviewed: bool) {
        let data = self.state.data.as_ref().expect("synthetic table");
        self.state.table_selection.select(data, 0, 0);
        let target = self
            .state
            .current_delete_target()
            .expect("synthetic selected row");
        self.mock_review_delete(target, reviewed);
    }

    pub(super) fn mock_review_delete(&mut self, target: DeleteTarget, publish: bool) {
        if !self.state.can_delete_row(&target) {
            return;
        }
        let data = self.state.data.as_ref().expect("admitted selected row");
        let Some(position) = data.row_index(target.parent_ordinal) else {
            return;
        };
        // DeleteReview owns independent column metadata while source Data stays live until
        // actual save publication; preserves original row review context.
        let columns = data.columns.clone();
        let row = data.rows[position].iter().map(copy_cell).collect();
        let request = self.request();
        // Checking owner retains target and simulated final DeleteReview must own same target
        // independently; final review consumes original.
        self.state.begin_delete(target.clone(), request);
        if publish {
            self.state.set_delete_review(
                request,
                DeleteReview {
                    id: request,
                    target,
                    columns,
                    row,
                    deleted_rows: 1,
                },
            );
        }
    }

    pub(super) fn mock_apply_delete(&mut self, review_id: u64) {
        if self.state.can_apply_delete(review_id) {
            let save = self.request();
            self.state.begin_delete_save(review_id, save);
            self.state.set_delete_save_outcome(
                save,
                EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
            );
        }
    }

    pub(super) fn mock_close_delete(&mut self, request_id: u64) {
        if matches!(&self.state.deletion,
            Some(Dialog::Checking { request_id: current, .. } | Dialog::Review { request_id: current, .. })
                if *current == request_id)
        {
            self.state.close_delete();
        }
    }

    pub(super) fn deletion_save_action(&mut self, action: &Action) -> bool {
        let Some(dialog) = &mut self.state.deletion else {
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
                    self.state.close_delete();
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
                self.state.begin_delete_resolution(original, operation);
                self.state.set_delete_save_outcome(
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
                self.state.begin_delete_resolution(original, operation);
                self.state.set_delete_save_outcome(
                    operation,
                    EditSaveOutcome::Failed(self.state.localizer.text("settings-mock-resolved")),
                );
                true
            }
            _ => false,
        }
    }
}
