//! No-lock Number review binds the exact original Required(false)/zero request.
pub(in crate::editing) mod availability;

use super::field_addition::plan::Plan;
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        AddColumnReview, AddedColumnReopen, EditError, ReviewedNumberColumn,
        platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{CreatedDefaultZeroColumnRequired, NativeImage};

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<AddedColumnReopen>,
    pub(in crate::editing) plan: Plan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    column: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_add_number_column").entered();
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
    // Select from actual native carrier metadata exactly once. Both captured
    // constructors retain their own complete proof; refusal never retries another.
    let external = image
        .number_column_uses_external_properties(table, &mut work, cancelled)
        .map_err(|error| Error::DefaultZeroAddColumn(error.into()))?;
    let plan = if external {
        Plan::Repeated(image.plan_recoverable_repeated_default_zero_int32_column(
            table, column, &mut work, cancelled,
        )?)
    } else {
        Plan::First(
            image.plan_recoverable_created_add_default_zero_int32_column(
                table,
                column,
                CreatedDefaultZeroColumnRequired::StoredFalse,
                &mut work,
                cancelled,
            )?,
        )
    };
    let expected = super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = AddColumnReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<AddedColumnReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(AddedColumnReopen {
        // NO_CLONE: The capability or reopen witness outlives this borrowed Document call and
        // owns its source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        document_revision: document.revision,
    });
    let capability = ReviewedNumberColumn {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::NumberColumnReviewed { review, capability })
}
