//! Lease-free explicit OFF review, bound to the unchanged complete source image.
pub(in crate::editing) mod plan;

use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
    editing::{
        DatabaseOptionsReopen, DatabaseOptionsReview, EditError, ReviewedDatabaseOptions,
        SaveDatabaseOptionsChange,
        platform::{ids::issue_id, reopen_image},
    },
};
use plan::Plan;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{DatabaseOptionsSourceError, DatabaseOptionsSourcePurpose, NativeImage};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<DatabaseOptionsReopen>,
    pub(in crate::editing) plan: Plan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_disable_name_auto_correct").entered();
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
    // Source purpose comes from the actual shared catalog-column ownership.
    // Exactly one constructor proves the full original; refusal never retries.
    let plan = match image.disable_name_auto_correct_source_purpose(&mut work, cancelled)? {
        DatabaseOptionsSourcePurpose::Creator => {
            Plan::Creator(image.plan_recoverable_disable_name_auto_correct(&mut work, cancelled)?)
        }
        DatabaseOptionsSourcePurpose::FieldHistory => Plan::FieldHistory(
            image.plan_recoverable_disable_name_auto_correct_after_field_history(
                &mut work, cancelled,
            )?,
        ),
    };
    let expected = reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let review = DatabaseOptionsReview {
        id: issue_id()?,
        document_revision: document.revision,
        change: SaveDatabaseOptionsChange::NameAutoCorrectDisabled,
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<DatabaseOptionsReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(DatabaseOptionsReopen {
        // NO_CLONE: The capability or reopen witness outlives this borrowed Document call and
        // owns its source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected,
        document_revision: document.revision,
    });
    let capability = ReviewedDatabaseOptions {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::DatabaseOptionsReviewed { review, capability })
}

/// A source proof for this fixed request; no candidate image or lease is created.
pub(in crate::editing) fn source_available(
    document: &Document,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Ok(false);
    }
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image =
        NativeImage::capture_shared(document.edits.image_source.clone(), SCOPE, work, cancelled)?;
    match image.check_disable_name_auto_correct_for_source(work, cancelled) {
        Ok(_) => Ok(true),
        Err(DatabaseOptionsSourceError::Source) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
