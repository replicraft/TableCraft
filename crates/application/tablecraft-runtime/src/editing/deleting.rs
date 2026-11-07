//! Source-bound single-row deletion review without an offline lease.
use crate::{Cell, Column};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use {
    super::EditError,
    crate::{
        Error, Response,
        controls::{SCOPE, budget},
        display::Presentation,
        document::Document,
    },
    bytes::Bytes,
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    tablecraft_access::editing::journal::NativeJournalEditor,
    tablecraft_ace::{NativeImage, RecoverableNativeDeletePlan},
    tablecraft_db::DeletableTable,
};

#[derive(Clone, PartialEq, Eq)]
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

pub struct ReviewedDelete {
    pub(super) state: Box<ReviewedDeleteState>,
}

pub(super) struct ReviewedDeleteState {
    pub(super) path: PathBuf,
    pub(super) target: DeleteTarget,
    refresh_ordinal: u32,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) original: Bytes,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) plan: RecoverableNativeDeletePlan,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) valid: Arc<AtomicBool>,
}

impl ReviewedDelete {
    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn target(&self) -> &DeleteTarget {
        &self.state.target
    }

    /// A display position in the remaining full relation, never a native handle.
    pub fn refresh_ordinal(&self) -> u32 {
        self.state.refresh_ordinal
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
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

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
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
        output.allocation(1, size_of::<ReviewedDeleteState>())?;
        let capability = ReviewedDelete {
            state: Box::new(ReviewedDeleteState {
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
                id: super::ids::issue_id()?,
                target,
                columns,
                row,
                deleted_rows: 1,
            },
            capability,
        })
    }
}
