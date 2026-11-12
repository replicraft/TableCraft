//! The named original DROP selects the sealed TCID plan before any save exists.
use crate::{
    Error, Response,
    controls::{SCOPE, action_review_budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, IndexDroppedReopen, ReviewedIndexDrop, platform::ids::issue_id},
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{NativeImage, RecoverableNativeIndexDropPlan};

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<IndexDroppedReopen>,
    pub(in crate::editing) plan: RecoverableNativeIndexDropPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    index: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }

    let mut work = action_review_budget(document.edits.original.len())?;
    // NO_CLONE: The source-issued plan outlives this borrowed Document call.
    // Sharing its immutable native owner avoids another complete database copy.
    let image = NativeImage::capture_shared_retained(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let plan = image.plan_recoverable_retained_index_drop(table, index, &mut work, cancelled)?;
    let intent = plan.intent();

    let mut output = Presentation::new(cancelled);
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<IndexDroppedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;

    let reopen = Arc::new(IndexDroppedReopen {
        // NO_CLONE: The acknowledged reopen keeps its path independently of the
        // still-open original Document; only the small filesystem path is copied.
        path: document.edits.path.clone(),
        expected: plan.after_shared_bytes(),
        document_revision: document.revision,
        table: output.text(plan.table())?,
        index: output.text(plan.index_name())?,
        native_table_id: intent.native_table_id(),
        definition_page: intent.definition_page(),
        logical_index_number: intent.logical_index_number(),
        physical_index_number: intent.physical_index_number(),
    });

    check(cancelled)?;

    Ok(Response::IndexDropReviewed {
        capability: ReviewedIndexDrop {
            state: Box::new(State {
                id: issue_id()?,
                reopen,
                plan,
                valid: Arc::clone(&document.edits.valid),
            }),
        },
    })
}
