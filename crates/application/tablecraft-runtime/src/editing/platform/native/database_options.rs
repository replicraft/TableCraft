//! Lease-free explicit OFF review, bound to the unchanged complete source image.
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
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{
    DatabaseOptionsError, DatabaseOptionsIntentError, NativeImage,
    RecoverableNativeDatabaseOptionsPlan,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<DatabaseOptionsReopen>,
    pub(in crate::editing) plan: RecoverableNativeDatabaseOptionsPlan,
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
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let plan = image.plan_recoverable_disable_name_auto_correct(&mut work, cancelled)?;
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
    let image =
        NativeImage::capture_shared(document.edits.image_source.clone(), SCOPE, work, cancelled)?;
    match image.check_disable_name_auto_correct_source(work, cancelled) {
        Ok(()) => Ok(true),
        Err(DatabaseOptionsError::Source) => Ok(false),
        Err(error) => Err(Error::DatabaseOptions(DatabaseOptionsIntentError::Options(
            error,
        ))),
    }
}
