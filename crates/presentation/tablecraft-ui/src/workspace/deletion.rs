//! Source-bound single-row deletion confirmation; runtime owns native admission.
mod dialog;

use super::{DocumentView, EditSaveOutcome, State};
use crate::{Cell, Column};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteTarget {
    pub document_revision: u64,
    pub table: String,
    pub parent_ordinal: u32,
}

pub struct DeleteReview {
    pub id: u64,
    pub target: DeleteTarget,
    pub columns: Vec<Column>,
    pub row: Vec<Cell>,
    pub deleted_rows: usize,
}

pub(super) enum Dialog {
    Checking {
        target: DeleteTarget,
        request_id: u64,
        error: Option<String>,
    },
    Review {
        review: Box<DeleteReview>,
        request_id: u64,
        error: Option<String>,
        focus_cancel: bool,
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

impl State {
    fn delete_source_current(&self, target: &DeleteTarget) -> bool {
        self.view == DocumentView::Table
            && !self.backstage_visible()
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == target.document_revision
                    && database
                        .tables
                        .iter()
                        .any(|table| table.name == target.table && !table.system)
            })
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(target.document_revision)
                    && data.table == target.table
                    && target
                        .parent_ordinal
                        .checked_sub(data.offset)
                        .is_some_and(|row| data.rows.get(row as usize).is_some())
            })
    }

    pub(super) fn refresh_delete_display(&mut self) {
        if let Some(Dialog::Review { review, .. }) = &mut self.deletion {
            for cell in &mut review.row {
                self.format_cache.apply(cell);
            }
        }
    }

    pub fn current_delete_target(&self) -> Option<DeleteTarget> {
        let data = self.data.as_ref()?;
        let (row, _) = self.table_selection.position()?;
        data.rows.get(row)?;
        let target = DeleteTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
            parent_ordinal: data.offset.checked_add(u32::try_from(row).ok()?)?,
        };
        self.can_delete_row(&target).then_some(target)
    }

    pub fn can_delete_row(&self, target: &DeleteTarget) -> bool {
        !self.busy
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && self.delete_source_current(target)
    }

    pub fn begin_delete(&mut self, target: DeleteTarget, request_id: u64) {
        if self.can_delete_row(&target) {
            self.reset_record_navigation();
            self.deletion = Some(Dialog::Checking {
                target,
                request_id,
                error: None,
            });
        }
    }

    pub fn set_delete_review(&mut self, request_id: u64, mut review: DeleteReview) -> bool {
        let accepts = matches!(&self.deletion,
            Some(Dialog::Checking { target, request_id: expected, .. })
                if *expected == request_id && *target == review.target);
        if !accepts || !self.delete_source_current(&review.target) {
            return false;
        }
        if review.deleted_rows != 1
            || review.columns.is_empty()
            || review.columns.len() != review.row.len()
        {
            self.fail_delete(request_id, self.localizer.text("deletion-invalid-review"));
            return false;
        }
        for cell in &mut review.row {
            self.format_cache.apply(cell);
        }

        self.deletion = Some(Dialog::Review {
            review: Box::new(review),
            request_id,
            error: None,
            focus_cancel: true,
        });
        true
    }

    pub fn can_apply_delete(&self, review_id: u64) -> bool {
        matches!(&self.deletion,
            Some(Dialog::Review { review, .. })
                if !self.busy && review.id == review_id
                    && self.delete_source_current(&review.target))
    }

    pub fn fail_delete(&mut self, request_id: u64, message: String) {
        if let Some(Dialog::Checking {
            request_id: expected,
            error,
            ..
        }) = &mut self.deletion
            && *expected == request_id
        {
            *error = Some(message);
        }
    }

    pub fn fail_delete_apply(&mut self, review_id: u64, message: String) {
        if let Some(Dialog::Review { review, error, .. }) = &mut self.deletion
            && review.id == review_id
        {
            *error = Some(message);
        }
    }
    pub fn begin_delete_save(&mut self, review_id: u64, save_id: u64) {
        if self.can_apply_delete(review_id) {
            self.deletion = Some(Dialog::Saving {
                save_id,
                cancel_requested: false,
                resolving: false,
            });
        }
    }

    pub fn begin_delete_resolution(&mut self, save_id: u64, operation_id: u64) {
        if matches!(&self.deletion,
            Some(Dialog::Finished { save_id: expected, outcome: EditSaveOutcome::Retained { .. } })
                if *expected == save_id)
        {
            self.deletion = Some(Dialog::Saving {
                save_id: operation_id,
                cancel_requested: false,
                resolving: true,
            });
        }
    }

    pub fn set_delete_save_outcome(&mut self, save_id: u64, outcome: EditSaveOutcome) {
        if matches!(&self.deletion,
            Some(Dialog::Saving { save_id: expected, .. })
            | Some(Dialog::Finished { save_id: expected, .. }) if *expected == save_id)
        {
            self.deletion = Some(Dialog::Finished { save_id, outcome });
        }
    }

    pub fn deletion_dialog_open(&self) -> bool {
        self.deletion.is_some()
    }

    pub fn delete_save_pending(&self) -> bool {
        matches!(
            &self.deletion,
            Some(Dialog::Saving { .. })
                | Some(Dialog::Finished {
                    outcome: EditSaveOutcome::Retained { .. },
                    ..
                })
        )
    }

    pub fn close_delete(&mut self) -> bool {
        if self.delete_save_pending() {
            return false;
        }
        self.deletion = None;
        true
    }

    pub(super) fn invalidate_delete(&mut self) {
        if matches!(
            &self.deletion,
            Some(Dialog::Checking { .. } | Dialog::Review { .. })
        ) {
            self.deletion = None;
        }
    }
}

#[cfg(test)]
mod tests;
