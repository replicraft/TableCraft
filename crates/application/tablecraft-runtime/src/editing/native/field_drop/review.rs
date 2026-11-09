//! One native source/request plan supplies both review facts and consuming owner.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, FieldDropReview, FieldDroppedReopen, ReviewedFieldDrop, platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{NativeImage, RecoverableFieldDropPlan};

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<FieldDroppedReopen>,
    pub(in crate::editing) plan: RecoverableFieldDropPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_field_drop").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut work = budget();
    check(cancelled)?;
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let plan = image.plan_recoverable_saved_field_drop(table, column, &mut work, cancelled)?;
    if plan.table() != table
        || plan.column() != column
        || plan.identity().column_number() != native_number
    {
        return Err(Error::StaleFieldDropTarget);
    }
    let expected = super::super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = FieldDropReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<FieldDroppedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(FieldDroppedReopen {
        // NO_CLONE: The capability or reopen witness outlives this borrowed Document call and
        // owns its source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
    });
    let capability = ReviewedFieldDrop {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::FieldDropReviewed { review, capability })
}
