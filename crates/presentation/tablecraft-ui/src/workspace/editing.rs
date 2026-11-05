//! Offline edit UI retains runtime-issued drafts/reviews, never native values or SQL.
mod dialog;
mod hints;
pub(super) mod paste;
pub use hints::InputHint;

use super::{Data, DocumentView, State};
use crate::{Cell, CellKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub parent_ordinal: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditInput {
    pub text: String,
    pub is_null: bool,
}

pub struct EditDraft {
    pub id: u64,
    pub target: EditTarget,
    pub original: Cell,
    pub initial: EditInput,
    pub input_hint: InputHint,
    pub nullable: bool,
}

pub struct EditReview {
    pub id: u64,
    pub draft_id: u64,
    pub target: EditTarget,
    pub before: Cell,
    pub after: Cell,
    pub changed_rows: usize,
}

pub enum EditSaveOutcome {
    Retained {
        message: String,
        can_recover: bool,
        can_release: bool,
    },
    Saved(String),
    Cancelled(String),
    Failed(String),
}

pub(super) enum EditDialog {
    Checking {
        target: EditTarget,
        request_id: u64,
        error: Option<String>,
        paste_seed: Option<String>,
    },
    Draft {
        draft: Box<EditDraft>,
        input: EditInput,
        request_id: u64,
        pending: Option<EditInput>,
        review: Option<Box<EditReview>>,
        reviewed_input: Option<EditInput>,
        error: Option<String>,
    },
    Saving {
        save_id: u64,
        cancel_requested: bool,
        resolving: bool,
    },
    Finished {
        save_id: u64,
        outcome: EditSaveOutcome,
    },
}

impl Data {
    pub(crate) fn edit_target(&self, row: usize, column: usize) -> Option<EditTarget> {
        let cell = self.rows.get(row)?.get(column)?;
        if !matches!(
            cell.kind,
            CellKind::Scalar {
                stored_calculation: false,
                ..
            }
        ) {
            return None;
        }
        Some(EditTarget {
            document_revision: self.source_revision?,
            table: self.table.clone(),
            column: self.columns.get(column)?.name.clone(),
            parent_ordinal: self.parent_ordinal(row)?,
        })
    }
}

impl State {
    pub(super) fn refresh_edit_display(&mut self) {
        if let Some(EditDialog::Draft { draft, review, .. }) = &mut self.edit {
            self.format_cache.apply(&mut draft.original);
            if let Some(review) = review {
                self.format_cache.apply(&mut review.before);
                self.format_cache.apply(&mut review.after);
            }
        }
    }

    /// A candidate for fresh backend admission, never a write permission grant.
    pub fn can_edit_cell(&self, target: &EditTarget) -> bool {
        if self.view != DocumentView::Table
            || self.zoom_dialog_open()
            || self.insertion_dialog_open()
            || self.deletion_dialog_open()
            || self.table_selection.row_selected()
            || self.backstage_visible()
            || self.contents_request_id().is_some()
            || self.database.as_ref().map(|db| db.document_revision)
                != Some(target.document_revision)
        {
            return false;
        }
        let Some(data) = &self.data else {
            return false;
        };
        let Some(row) = data.row_index(target.parent_ordinal) else {
            return false;
        };
        let Some(column) = data.column_index(&target.column) else {
            return false;
        };
        data.source_revision == Some(target.document_revision)
            && data.table == target.table
            && data
                .rows
                .get(row)
                .and_then(|row| row.get(column))
                .is_some_and(|cell| {
                    matches!(
                        cell.kind,
                        CellKind::Scalar {
                            stored_calculation: false,
                            ..
                        }
                    )
                })
    }

    pub fn begin_edit(&mut self, target: EditTarget, request_id: u64) {
        if !self.edit_save_pending() && self.can_edit_cell(&target) {
            self.reset_record_navigation();
            self.edit = Some(EditDialog::Checking {
                target,
                request_id,
                error: None,
                paste_seed: None,
            });
        }
    }

    pub fn set_edit_draft(&mut self, request_id: u64, draft: EditDraft) {
        let accepts = matches!(&self.edit, Some(EditDialog::Checking { target, request_id: expected, .. })
            if *expected == request_id && target == &draft.target);
        if accepts && self.can_edit_cell(&draft.target) {
            let seed = match &mut self.edit {
                Some(EditDialog::Checking { paste_seed, .. }) => paste_seed.take(),
                _ => None,
            };
            let input = seed.map_or_else(
                || draft.initial.clone(),
                |text| EditInput {
                    text,
                    is_null: false,
                },
            );
            self.edit = Some(EditDialog::Draft {
                draft: Box::new(draft),
                input,
                request_id,
                pending: None,
                review: None,
                reviewed_input: None,
                error: None,
            });
        }
    }

    /// Admit only the current draft input before replacing a read request.
    pub fn can_review_edit(&self, draft_id: u64, input: &EditInput) -> bool {
        let Some(EditDialog::Draft {
            draft,
            input: current,
            pending: None,
            ..
        }) = &self.edit
        else {
            return false;
        };
        draft.id == draft_id && current == input && self.can_edit_cell(&draft.target)
    }

    pub fn begin_edit_review(&mut self, draft_id: u64, input: EditInput, request_id: u64) {
        if let Some(EditDialog::Draft {
            draft,
            input: current,
            pending,
            review,
            request_id: active,
            reviewed_input,
            error,
        }) = &mut self.edit
            && draft.id == draft_id
            && current == &input
        {
            *active = request_id;
            *pending = Some(input);
            *review = None;
            *reviewed_input = None;
            *error = None;
        }
    }

    /// Accept only the outstanding review of this exact still-visible input.
    pub fn set_edit_review(&mut self, request_id: u64, reviewed: EditReview) -> bool {
        if !self.can_edit_cell(&reviewed.target) {
            return false;
        }
        if let Some(EditDialog::Draft {
            draft,
            input,
            pending,
            review,
            request_id: expected,
            reviewed_input,
            error,
        }) = &mut self.edit
            && *expected == request_id
            && draft.id == reviewed.draft_id
            && draft.target == reviewed.target
            && pending.as_ref() == Some(input)
        {
            *reviewed_input = pending.take();
            *review = Some(Box::new(reviewed));
            *error = None;
            return true;
        }
        false
    }

    /// Check the still-visible accepted input before consuming a save capability.
    pub fn can_apply_edit(&self, review_id: u64) -> bool {
        let Some(EditDialog::Draft {
            draft,
            input,
            pending: None,
            review: Some(review),
            reviewed_input: Some(reviewed_input),
            ..
        }) = &self.edit
        else {
            return false;
        };
        review.id == review_id
            && review.draft_id == draft.id
            && review.target == draft.target
            && input == reviewed_input
            && self.can_edit_cell(&review.target)
    }

    /// Starting the save failed before capability consumption; keep the review.
    pub fn fail_edit_apply(&mut self, review_id: u64, message: String) {
        if let Some(EditDialog::Draft { review, error, .. }) = &mut self.edit
            && review.as_ref().is_some_and(|review| review.id == review_id)
        {
            *error = Some(message);
        }
    }

    pub fn fail_edit(&mut self, request_id: u64, message: String) {
        match &mut self.edit {
            Some(EditDialog::Checking {
                request_id: expected,
                error,
                ..
            }) if *expected == request_id => *error = Some(message),
            Some(EditDialog::Draft {
                request_id: expected,
                pending,
                review,
                error,
                ..
            }) if *expected == request_id && pending.is_some() => {
                *pending = None;
                *review = None;
                *error = Some(message);
            }
            _ => {}
        }
    }

    /// Called only when the host has consumed a real reviewed capability into SaveWorker.
    pub fn begin_edit_save(&mut self, review_id: u64, save_id: u64) {
        let review = match &mut self.edit {
            Some(EditDialog::Draft { review, .. })
                if review.as_ref().is_some_and(|review| review.id == review_id) =>
            {
                review.take()
            }
            _ => None,
        };
        if review.is_some() {
            self.edit = Some(EditDialog::Saving {
                save_id,
                cancel_requested: false,
                resolving: false,
            });
        }
    }

    /// The retained actor has accepted a recovery or release operation.
    pub fn begin_edit_resolution(&mut self, save_id: u64, operation_id: u64) {
        if matches!(&self.edit, Some(EditDialog::Finished { save_id: expected, outcome: EditSaveOutcome::Retained { .. } }) if *expected == save_id)
        {
            self.edit = Some(EditDialog::Saving {
                save_id: operation_id,
                cancel_requested: false,
                resolving: true,
            });
        }
    }

    /// Terminal save outcomes are retained even when the original page is replaced.
    pub fn set_edit_save_outcome(&mut self, save_id: u64, outcome: EditSaveOutcome) {
        if matches!(&self.edit, Some(EditDialog::Saving { save_id: expected, .. }) | Some(EditDialog::Finished { save_id: expected, .. }) if *expected == save_id)
        {
            self.edit = Some(EditDialog::Finished { save_id, outcome });
        }
    }

    pub fn edit_save_pending(&self) -> bool {
        self.insert_save_pending()
            || self.delete_save_pending()
            || matches!(
                &self.edit,
                Some(EditDialog::Saving { .. })
                    | Some(EditDialog::Finished {
                        outcome: EditSaveOutcome::Retained { .. },
                        ..
                    })
            )
    }
    pub fn edit_dialog_open(&self) -> bool {
        self.edit.is_some() || self.insertion_dialog_open() || self.deletion_dialog_open()
    }

    /// Saving is nonreplaceable: close/cancel must wait for a terminal outcome.
    pub fn close_edit(&mut self) -> bool {
        if self.edit_save_pending() {
            return false;
        }
        self.edit = None;
        true
    }

    pub(super) fn edit_status(&self) -> String {
        if self.deletion_dialog_open() {
            return self.localizer.text(if self.delete_save_pending() {
                "shell-saving-offline"
            } else {
                "deletion-title"
            });
        }
        if self.insertion_dialog_open() {
            return self.localizer.text(
                if matches!(
                    self.insertion,
                    Some(super::insertion::Dialog::Saving { .. })
                ) {
                    "shell-saving-offline"
                } else {
                    "insertion-title"
                },
            );
        }
        match &self.edit {
            Some(EditDialog::Saving { .. }) => self.localizer.text("shell-saving-offline"),
            Some(_) => self.localizer.text("shell-offline-edit"),
            None => self.localizer.text("shell-snapshot-browsing"),
        }
    }

    pub(super) fn invalidate_edit(&mut self) {
        self.invalidate_insert();
        self.invalidate_delete();
        if matches!(
            &self.edit,
            Some(EditDialog::Checking { .. } | EditDialog::Draft { .. })
        ) {
            self.edit = None;
        }
    }
}

#[cfg(test)]
mod tests;
