//! Source-bound preparation and ordinary planning without a file lease.
use super::ids::issue_id;
use super::{
    EditDraft, EditError, EditInput, EditReview, EditTarget, ReviewedEdit, ReviewedState, input,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    scalar_input::InputHint,
};
use std::sync::Arc;
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::Resource;
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};

fn copy_target(target: &EditTarget, output: &mut Presentation<'_>) -> Result<EditTarget, Error> {
    Ok(EditTarget {
        document_revision: target.document_revision,
        table: output.text(&target.table)?,
        column: output.text(&target.column)?,
        parent_ordinal: target.parent_ordinal,
    })
}

impl Document {
    pub(crate) fn begin_edit(
        &mut self,
        target: &EditTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let mut work = budget();
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let editor = NativeJournalEditor::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
        if editor.update_profile() == UpdateProfile::Unavailable {
            return Err(EditError::Unavailable.into());
        }

        let snapshot = editor.snapshot();
        let mut column = None;
        for (index, field) in snapshot.columns().iter().enumerate() {
            check(cancelled)?;
            work.charge(
                Resource::Steps,
                1 + field.name().len() as u64 + target.column.len() as u64,
            )?;
            if field.name() == target.column {
                column = Some(index);
                break;
            }
        }
        let index = column.ok_or(EditError::TargetMissing)?;
        let field = &snapshot.columns()[index];
        let original = snapshot
            .rows()
            .get(target.parent_ordinal as usize)
            .and_then(|row| row.get(index))
            .ok_or(EditError::TargetMissing)?;

        let mut output = Presentation::new(cancelled);
        let initial = input::initial(original, &mut output)?;
        let original = output.scalar(original, false)?;
        let input_hint = InputHint::from(field.data_type());
        let id = issue_id()?;
        let retained = copy_target(target, &mut output)?;
        let target = copy_target(target, &mut output)?;
        self.edits.draft = Some((id, retained));

        Ok(Response::EditDraft(EditDraft {
            id,
            target,
            original,
            initial,
            input_hint,
            nullable: field.nullable(),
        }))
    }

    pub(crate) fn review_edit(
        &mut self,
        draft_id: u64,
        input: &EditInput,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let (id, target) = self.edits.draft.as_ref().ok_or(EditError::StaleDraft)?;
        if *id != draft_id || target.document_revision != self.revision {
            return Err(EditError::StaleDraft.into());
        }
        let mut work = budget();
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let editor = NativeJournalEditor::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;

        let snapshot = editor.snapshot();
        let mut index = None;
        for (position, field) in snapshot.columns().iter().enumerate() {
            check(cancelled)?;
            work.charge(
                Resource::Steps,
                1 + field.name().len() as u64 + target.column.len() as u64,
            )?;
            if field.name() == target.column {
                index = Some(position);
                break;
            }
        }
        let index = index.ok_or(EditError::TargetMissing)?;
        let handle = snapshot
            .row_handle(target.parent_ordinal as usize)
            .ok_or(EditError::TargetMissing)?;
        let original = &snapshot.rows()[target.parent_ordinal as usize][index];

        let mut output = Presentation::new(cancelled);
        let kind = snapshot.columns()[index].data_type();
        let replacement = input::parse_original(kind, input, original, &mut output)?;
        let before = output.scalar(original, false)?;
        let after = output.scalar(&replacement, false)?;
        let retained_replacement = input::parse_original(kind, input, original, &mut output)?;

        let plan = editor
            .plan_updates(
                &target.column,
                &[CellUpdate::new(handle, replacement)],
                &mut work,
                cancelled,
            )
            .map_err(EditError::from)?;
        // The consuming normal plan proves the change. Reparse the same bounded
        // input for the fresh leased planner; never retain a foreign row handle.
        let replacement = retained_replacement;
        let review_target = copy_target(target, &mut output)?;
        let capability_target = copy_target(target, &mut output)?;
        let changed_rows = plan.changed_rows();
        work.charge_allocation(self.edits.path.as_os_str().len() as u64, 1)?;
        work.charge_allocation(1, size_of::<ReviewedState>() as u64)?;
        let capability = ReviewedEdit {
            state: Box::new(ReviewedState {
                path: self.edits.path.clone(),
                original: self.edits.original.clone(),
                target: capability_target,
                replacement,
                plan,
                valid: Arc::clone(&self.edits.valid),
            }),
        };

        Ok(Response::EditReviewed {
            review: EditReview {
                id: issue_id()?,
                draft_id,
                target: review_target,
                before,
                after,
                changed_rows,
            },
            capability,
        })
    }
}
