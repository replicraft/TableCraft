//! One native source/request plan supplies both review facts and consuming owner.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, FieldRenameReview, FieldRenamedReopen, ReviewedFieldRename,
        platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{NativeImage, RecoverableFieldRenamePlan};

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<FieldRenamedReopen>,
    pub(in crate::editing) plan: RecoverableFieldRenamePlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    new_name: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_field_rename").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut work = budget();
    check(cancelled)?;
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let plan = image
        .plan_recoverable_same_width_field_rename(table, column, new_name, &mut work, cancelled)?;
    if plan.table() != table
        || plan.column() != column
        || plan.identity().column_number() != native_number
    {
        return Err(Error::StaleFieldRenameTarget);
    }
    let expected = super::super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = FieldRenameReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
        new_name: output.text(plan.new_name())?,
        changed: plan.changed(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<FieldRenamedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(FieldRenamedReopen {
        path: document.edits.path.clone(),
        expected,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
        new_name: output.text(plan.new_name())?,
        changed: plan.changed(),
    });
    let capability = ReviewedFieldRename {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::FieldRenameReviewed { review, capability })
}
