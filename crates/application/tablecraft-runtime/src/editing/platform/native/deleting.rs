//! Native deletion review and its source-bound private capability.
use crate::editing::{DeleteReview, DeleteTarget, ReviewedDelete};
use crate::{
    Error, Response,
    controls::{SCOPE, budget},
    display::Presentation,
    document::Document,
    editing::EditError,
};
use bytes::Bytes;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::{NativeImage, RecoverableNativeDeletePlan};
use tablecraft_db::DeletableTable;

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: DeleteTarget,
    pub(in crate::editing) refresh_ordinal: u32,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) plan: RecoverableNativeDeletePlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

fn copy_target(
    target: &DeleteTarget,
    output: &mut Presentation<'_>,
) -> Result<DeleteTarget, Error> {
    Ok(DeleteTarget {
        document_revision: target.document_revision,
        table: output.text(&target.table)?,
        parent_ordinal: target.parent_ordinal,
    })
}

impl Document {
    pub(crate) fn review_delete(
        &mut self,
        target: &DeleteTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if target.document_revision != self.revision || !self.edits.valid.load(Ordering::Acquire) {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }

        let mut work = budget();
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let metadata = crate::rows::presentation::Metadata::inspect(
            &mut image.reader(),
            &target.table,
            &mut work,
            cancelled,
        )?;
        let editor = NativeJournalEditor::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
        let snapshot = editor.snapshot();
        let handle = snapshot
            .row_handle(target.parent_ordinal as usize)
            .ok_or(EditError::TargetMissing)?;
        // The editor owns the complete, EOF-checked native snapshot. Use its
        // actual membership, never the table's declared row count or a page.
        let remaining_last = snapshot.rows().len().saturating_sub(2);
        let refresh_ordinal = u32::try_from((target.parent_ordinal as usize).min(remaining_last))
            .map_err(|_| EditError::Unavailable)?;
        let original = snapshot
            .rows()
            .get(target.parent_ordinal as usize)
            .ok_or(EditError::TargetMissing)?;

        let mut output = Presentation::new(cancelled);
        let mut columns = output.columns(snapshot.columns().len())?;
        for (position, field) in snapshot.columns().iter().enumerate() {
            columns.push(metadata.column(position, field.name(), &mut output)?);
        }

        let mut row = output.row(original.len())?;
        for value in original {
            row.push(output.scalar(value, false)?);
        }

        let mut rows = output.rows(1)?;
        rows.push(row);
        metadata.enrich(&columns, &mut rows, &mut output)?;
        let row = rows.pop().ok_or(EditError::TargetMissing)?;

        let plan = editor
            .plan_deletes(&[handle], &mut work, cancelled)
            .map_err(EditError::from)?;
        if plan.deleted_rows() != 1 {
            return Err(EditError::Unavailable.into());
        }

        let capability_target = copy_target(target, &mut output)?;
        let target = copy_target(target, &mut output)?;
        output.allocation(self.edits.path.as_os_str().len(), 1)?;
        output.allocation(1, size_of::<State>())?;
        let capability = ReviewedDelete {
            state: Box::new(State {
                path: self.edits.path.clone(),
                target: capability_target,
                refresh_ordinal,
                original: self.edits.original.clone(),
                plan,
                valid: Arc::clone(&self.edits.valid),
            }),
        };

        Ok(Response::DeleteReviewed {
            review: DeleteReview {
                id: crate::editing::platform::ids::issue_id()?,
                target,
                columns,
                row,
                deleted_rows: 1,
            },
            capability,
        })
    }
}
