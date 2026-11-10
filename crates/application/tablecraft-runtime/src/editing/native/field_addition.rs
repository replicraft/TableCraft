//! Exact field declaration review; select source purpose before any qualification.
pub(in crate::editing) mod availability;
pub(in crate::editing) mod plan;
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        AddedColumnReopen, EditError, FieldAdditionReview, ReviewedFieldAddition,
        platform::ids::issue_id,
    },
};
use plan::Plan;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{
    CreatedDefaultZeroColumnRequired, CreatedFieldAdditionSourcePurpose,
    FieldAdditionSourcePurpose, FieldAdditionTarget, NativeImage,
};
use tablecraft_builders::table::FieldAdditionDeclaration;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<AddedColumnReopen>,
    pub(in crate::editing) plan: Plan,
    pub(in crate::editing) declaration: FieldAdditionDeclaration,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

fn native_target(declaration: FieldAdditionDeclaration) -> FieldAdditionTarget {
    match declaration {
        FieldAdditionDeclaration::NumberLongZero => FieldAdditionTarget::NumberLongZero,
        FieldAdditionDeclaration::ShortText {
            max_utf16_units,
            unicode_compression,
        } => FieldAdditionTarget::ShortText {
            max_utf16_units: usize::from(max_utf16_units.get()),
            unicode_compression,
        },
    }
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    column: &str,
    declaration: FieldAdditionDeclaration,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_add_field").entered();
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
    let target = native_target(declaration);
    let purpose = image.field_addition_source_purpose(table, target, &mut work, cancelled)?;
    let plan = match purpose {
        FieldAdditionSourcePurpose::CurrentNumber => Plan::Current(Box::new(
            image.plan_recoverable_current_number_field_addition(
                table, column, &mut work, cancelled,
            )?,
        )),
        FieldAdditionSourcePurpose::CurrentShortText => {
            let FieldAdditionTarget::ShortText {
                max_utf16_units,
                unicode_compression,
            } = target
            else {
                return Err(EditError::InvalidInput.into());
            };
            Plan::Current(Box::new(
                image.plan_recoverable_current_short_text_field_addition(
                    table,
                    column,
                    max_utf16_units,
                    unicode_compression,
                    &mut work,
                    cancelled,
                )?,
            ))
        }
        FieldAdditionSourcePurpose::Created(CreatedFieldAdditionSourcePurpose::FirstNumber) => {
            Plan::First(
                image.plan_recoverable_created_add_default_zero_int32_column(
                    table,
                    column,
                    CreatedDefaultZeroColumnRequired::StoredFalse,
                    &mut work,
                    cancelled,
                )?,
            )
        }
        FieldAdditionSourcePurpose::Created(CreatedFieldAdditionSourcePurpose::RepeatedNumber) => {
            Plan::Repeated(image.plan_recoverable_repeated_default_zero_int32_column(
                table, column, &mut work, cancelled,
            )?)
        }
        FieldAdditionSourcePurpose::Created(CreatedFieldAdditionSourcePurpose::TypedField) => {
            Plan::Typed(image.plan_recoverable_created_field_addition(
                table, column, target, &mut work, cancelled,
            )?)
        }
        FieldAdditionSourcePurpose::AfterExplicitOff => {
            Plan::AfterOff(image.plan_recoverable_created_field_addition_after_off(
                table, column, target, &mut work, cancelled,
            )?)
        }
        FieldAdditionSourcePurpose::AfterExplicitOffHistory => Plan::AfterOffHistory(
            image.plan_recoverable_created_field_addition_after_off_history(
                table, column, target, &mut work, cancelled,
            )?,
        ),
    };
    let captured = plan.captured_declaration()?;
    if captured != declaration {
        return Err(EditError::InvalidInput.into());
    }
    let expected = super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = FieldAdditionReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        declaration: captured,
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
    let capability = ReviewedFieldAddition {
        state: Box::new(State {
            reopen,
            declaration: captured,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::FieldAdditionReviewed { review, capability })
}
