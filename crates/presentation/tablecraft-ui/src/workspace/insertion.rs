//! Runtime-owned row insertion decisions, independent of cell-edit row handles.
mod dialog;
pub(crate) mod inline;

use super::{DocumentView, EditInput, EditSaveOutcome, InputHint, State};

const MAX_INSERT_FIELDS: usize = 256;
const MAX_INSERT_TEXT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertTarget {
    pub document_revision: u64,
    pub table: String,
}

pub struct InsertField {
    pub name: String,
    pub input_hint: InputHint,
    pub nullable: bool,
    /// Stored AutoNumber metadata; generator support is separately qualified.
    pub auto_number: bool,
}

pub struct InsertDraft {
    pub id: u64,
    pub target: InsertTarget,
    pub fields: Vec<InsertField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertInput {
    Omitted,
    Value(EditInput),
}

pub struct InsertReview {
    pub id: u64,
    pub draft_id: u64,
    pub target: InsertTarget,
    pub inputs: Vec<InsertInput>,
    pub inserted_rows: usize,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Choice {
    #[default]
    Omitted,
    Null,
    Value,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct FieldInput {
    choice: Choice,
    text: String,
    input_rejected: bool,
}

impl FieldInput {
    fn input(&self) -> InsertInput {
        match self.choice {
            Choice::Omitted => InsertInput::Omitted,
            Choice::Null => InsertInput::Value(EditInput {
                text: self.text.clone(),
                is_null: true,
            }),
            Choice::Value => InsertInput::Value(EditInput {
                text: self.text.clone(),
                is_null: false,
            }),
        }
    }
}

pub(super) enum Dialog {
    Checking {
        target: InsertTarget,
        request_id: u64,
        error: Option<String>,
    },
    Draft {
        draft: Box<InsertDraft>,
        entries: Vec<FieldInput>,
        request_id: u64,
        pending: Option<Vec<InsertInput>>,
        review: Option<Box<InsertReview>>,
        error: Option<String>,
        inline: inline::Editor,
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

fn inputs(entries: &[FieldInput]) -> Vec<InsertInput> {
    entries.iter().map(FieldInput::input).collect()
}

fn within_input_limits(proposed: &[InsertInput]) -> bool {
    proposed.len() <= MAX_INSERT_FIELDS
        && text_bytes_fit(proposed.iter().filter_map(|input| match input {
            InsertInput::Omitted => None,
            InsertInput::Value(value) => Some(value.text.as_str()),
        }))
}

fn entries_within_limits(entries: &[FieldInput]) -> bool {
    entries.len() <= MAX_INSERT_FIELDS
        && entries.iter().all(|entry| !entry.input_rejected)
        && text_bytes_fit(entries.iter().map(|entry| entry.text.as_str()))
}

fn remaining_input_bytes(entries: &[FieldInput], position: usize) -> usize {
    entries
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != position)
        .try_fold(MAX_INSERT_TEXT_BYTES, |remaining, (_, entry)| {
            remaining.checked_sub(entry.text.len())
        })
        .unwrap_or(0)
}

fn text_bytes_fit<'a>(mut texts: impl Iterator<Item = &'a str>) -> bool {
    texts
        .try_fold(0_usize, |total, text| {
            total
                .checked_add(text.len())
                .filter(|total| *total <= MAX_INSERT_TEXT_BYTES)
        })
        .is_some()
}

fn generated_entries_omitted(fields: &[InsertField], entries: &[FieldInput]) -> bool {
    fields.len() == entries.len()
        && fields.iter().zip(entries).all(|(field, entry)| {
            !field.auto_number || (entry.choice == Choice::Omitted && entry.text.is_empty())
        })
}

fn inputs_match(fields: &[InsertField], entries: &[FieldInput], proposed: &[InsertInput]) -> bool {
    entries_within_limits(entries)
        && generated_entries_omitted(fields, entries)
        && entries.len() == proposed.len()
        && entries
            .iter()
            .zip(proposed)
            .all(|(entry, input)| match (entry.choice, input) {
                (Choice::Omitted, InsertInput::Omitted) => true,
                (Choice::Null, InsertInput::Value(value)) => {
                    value.is_null && value.text == entry.text
                }
                (Choice::Value, InsertInput::Value(value)) => {
                    !value.is_null && value.text == entry.text
                }
                _ => false,
            })
}

fn draft_columns_match(draft: &InsertDraft, data: &crate::Data) -> bool {
    draft.fields.iter().enumerate().all(|(index, field)| {
        !draft.fields[..index]
            .iter()
            .any(|prior| prior.name == field.name)
            && data
                .columns
                .iter()
                .filter(|column| column.name == field.name)
                .count()
                == 1
    })
}

impl State {
    fn insert_draft_current(&self, draft: &InsertDraft) -> bool {
        self.insert_source_current(&draft.target)
            && self
                .data
                .as_ref()
                .is_some_and(|data| draft_columns_match(draft, data))
    }

    fn insert_source_current(&self, target: &InsertTarget) -> bool {
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
                    && data.validate_row_ordinals().is_ok()
                    && data.table == target.table
            })
    }

    pub fn current_insert_target(&self) -> Option<InsertTarget> {
        let data = self.data.as_ref()?;
        let target = InsertTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
        };
        self.can_insert_row(&target).then_some(target)
    }

    pub fn can_insert_row(&self, target: &InsertTarget) -> bool {
        !self.busy
            && !self.zoom_dialog_open()
            && self.edit.is_none()
            && self.insertion.is_none()
            && self.deletion.is_none()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && self.insert_source_current(target)
    }

    pub fn begin_insert(&mut self, target: InsertTarget, request_id: u64) {
        if self.can_insert_row(&target) {
            self.reset_record_navigation();
            self.insertion = Some(Dialog::Checking {
                target,
                request_id,
                error: None,
            });
        }
    }

    pub fn set_insert_draft(&mut self, request_id: u64, draft: InsertDraft) {
        let accepts = matches!(&self.insertion,
            Some(Dialog::Checking { target, request_id: expected, .. })
                if *expected == request_id && target == &draft.target);
        if !accepts || !self.insert_source_current(&draft.target) {
            return;
        }
        if draft.fields.len() > MAX_INSERT_FIELDS {
            self.fail_insert(request_id, self.localizer.text("insertion-field-limit"));
            return;
        }
        let entries = draft.fields.iter().map(|_| FieldInput::default()).collect();
        self.insertion = Some(Dialog::Draft {
            draft: Box::new(draft),
            entries,
            request_id,
            inline: inline::Editor::default(),
            pending: None,
            review: None,
            error: None,
        });
    }

    pub fn can_review_insert(&self, draft_id: u64, proposed: &[InsertInput]) -> bool {
        let Some(Dialog::Draft {
            draft,
            entries,
            pending: None,
            ..
        }) = &self.insertion
        else {
            return false;
        };
        !self.busy
            && draft.id == draft_id
            && within_input_limits(proposed)
            && self.insert_draft_current(draft)
            && inputs_match(&draft.fields, entries, proposed)
    }

    pub fn begin_insert_review(
        &mut self,
        draft_id: u64,
        proposed: Vec<InsertInput>,
        request_id: u64,
    ) {
        if !self.can_review_insert(draft_id, &proposed) {
            return;
        }
        if let Some(Dialog::Draft {
            draft,
            entries,
            request_id: active,
            pending,
            review,
            error,
            ..
        }) = &mut self.insertion
            && draft.id == draft_id
            && inputs_match(&draft.fields, entries, &proposed)
        {
            *active = request_id;
            *pending = Some(proposed);
            *review = None;
            *error = None;
        }
    }

    pub fn set_insert_review(&mut self, request_id: u64, reviewed: InsertReview) -> bool {
        if !self.insert_source_current(&reviewed.target) {
            return false;
        }
        if let Some(Dialog::Draft {
            draft,
            entries,
            request_id: expected,
            pending,
            review,
            error,
            ..
        }) = &mut self.insertion
            && *expected == request_id
            && draft.id == reviewed.draft_id
            && draft.target == reviewed.target
            && pending.as_deref() == Some(reviewed.inputs.as_slice())
            && inputs_match(&draft.fields, entries, &reviewed.inputs)
        {
            *pending = None;
            *review = Some(Box::new(reviewed));
            *error = None;
            return true;
        }
        false
    }

    pub fn can_apply_insert(&self, review_id: u64) -> bool {
        let Some(Dialog::Draft {
            draft,
            entries,
            pending: None,
            review: Some(review),
            ..
        }) = &self.insertion
        else {
            return false;
        };
        !self.busy
            && self.insert_draft_current(draft)
            && review.id == review_id
            && review.draft_id == draft.id
            && review.target == draft.target
            && inputs_match(&draft.fields, entries, &review.inputs)
    }

    pub fn begin_insert_save(&mut self, review_id: u64, save_id: u64) {
        if self.can_apply_insert(review_id) {
            self.insertion = Some(Dialog::Saving {
                save_id,
                cancel_requested: false,
                resolving: false,
            });
        }
    }

    pub fn begin_insert_resolution(&mut self, save_id: u64, operation_id: u64) {
        if matches!(&self.insertion,
            Some(Dialog::Finished { save_id: expected, outcome: EditSaveOutcome::Retained { .. } })
                if *expected == save_id)
        {
            self.insertion = Some(Dialog::Saving {
                save_id: operation_id,
                cancel_requested: false,
                resolving: true,
            });
        }
    }

    pub fn set_insert_save_outcome(&mut self, save_id: u64, outcome: EditSaveOutcome) {
        if matches!(&self.insertion,
            Some(Dialog::Saving { save_id: expected, .. })
            | Some(Dialog::Finished { save_id: expected, .. }) if *expected == save_id)
        {
            self.insertion = Some(Dialog::Finished { save_id, outcome });
        }
    }

    pub fn fail_insert(&mut self, request_id: u64, message: String) {
        match &mut self.insertion {
            Some(Dialog::Checking {
                request_id: expected,
                error,
                ..
            }) if *expected == request_id => {
                *error = Some(message);
            }
            Some(Dialog::Draft {
                request_id: expected,
                pending,
                review,
                error,
                inline,
                ..
            }) if *expected == request_id && pending.is_some() => {
                inline.restore_focus();
                *pending = None;
                *review = None;
                *error = Some(message);
            }
            _ => {}
        }
    }

    pub fn fail_insert_apply(&mut self, review_id: u64, message: String) {
        if let Some(Dialog::Draft {
            review: Some(review),
            error,
            ..
        }) = &mut self.insertion
            && review.id == review_id
        {
            *error = Some(message);
        }
    }

    pub fn inline_insert_active(&self) -> bool {
        matches!(
            &self.insertion,
            Some(Dialog::Draft {
                pending: None,
                review: None,
                ..
            })
        )
    }

    pub(super) fn insertion_modal_open(&self) -> bool {
        self.insertion.is_some() && !self.inline_insert_active()
    }

    pub fn insertion_dialog_open(&self) -> bool {
        self.insertion.is_some()
    }

    pub fn insert_save_pending(&self) -> bool {
        matches!(
            &self.insertion,
            Some(Dialog::Saving { .. })
                | Some(Dialog::Finished {
                    outcome: EditSaveOutcome::Retained { .. },
                    ..
                })
        )
    }

    pub fn close_insert(&mut self) -> bool {
        if self.insert_save_pending() {
            return false;
        }
        self.insertion = None;
        true
    }

    pub(super) fn invalidate_insert(&mut self) {
        // A different publication invalidates permission, not the user's input.
        if let Some(Dialog::Draft {
            pending,
            review,
            error,
            ..
        }) = &mut self.insertion
        {
            *pending = None;
            *review = None;
            *error = Some(self.localizer.text("insertion-stale-draft"));
        } else if matches!(&self.insertion, Some(Dialog::Checking { .. })) {
            self.insertion = None;
        }
    }
}

#[cfg(test)]
mod tests;
