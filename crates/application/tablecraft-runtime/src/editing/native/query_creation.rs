//! Lease-free review of one actual source and one captured creation DATE.
use super::query_syntax::{parsed, syntax_budget};
use crate::{
    Error, QueryCreatedReopen, QueryCreationOrigin, QueryCreationReview, QueryCreationTarget,
    Response, ReviewedQueryCreation, SavedQueryTarget,
    controls::{SCOPE, action_review_budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, platform::ids::issue_id},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tablecraft_access::{
    NativeSavedReadError, SavedQueryCreationOptions, prepare_saved_query_creation,
    reconstruct_saved_read,
};
use tablecraft_ace::{
    InspectError, NativeImage, RecoverableNativeQueryCreationPlan, read_saved_query_definition,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::functions::dates::clock::{Clock, ClockFunction};
use tablecraft_model::ObjectKind;
use tablecraft_sql::ParseLimits;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<QueryCreatedReopen>,
    pub(in crate::editing) plan: RecoverableNativeQueryCreationPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

fn preflight(
    document: &Document,
    revision: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(cancelled)?;
    if revision != document.revision || !document.edits.valid.load(Ordering::Acquire) {
        return Err(Error::StaleDocument);
    }
    if !document.source.retained_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    Ok(())
}

fn capture(
    document: &Document,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, Error> {
    // NO_CLONE: The sealed native plan retains the immutable original after
    // this borrowed Document call; its reader keeps a separate shared owner.
    Ok(NativeImage::capture_shared_retained(
        document.edits.image_source.clone(),
        SCOPE,
        work,
        cancelled,
    )?)
}

pub(in crate::editing) fn review_sql(
    document: &mut Document,
    target: &mut QueryCreationTarget,
    sql: &Arc<String>,
    destination: &str,
    clock: &dyn Clock,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_query_creation").entered();
    preflight(document, target.document_revision, cancelled)?;
    let mut native = action_review_budget(document.edits.original.len())?;
    let mut syntax = syntax_budget();
    if let Some(source) = &target.source {
        preflight(document, source.document_revision, cancelled)?;
    }
    let image = capture(document, &mut native, cancelled)?;
    if let Some(source) = &target.source {
        validate_saved(&image, source, &mut native, cancelled)?;
    }
    let query = parsed(sql, &mut syntax, cancelled)?;
    check(cancelled)?;
    native.charge(Resource::Steps, 1)?;
    let Value::DateTime(captured_at) = ClockFunction::Now.apply(Some(clock))? else {
        return Err(Error::ParameterInput);
    };
    let plan = prepare_saved_query_creation(
        image,
        query.query(),
        SavedQueryCreationOptions {
            destination,
            captured_at,
            limits: ParseLimits::default(),
        },
        &mut native,
        &mut syntax,
        cancelled,
    )?;
    issue(document, plan, &mut native, cancelled, || {
        let mut output = Presentation::new(cancelled);
        if let Some(source) = &target.source {
            output.allocation(source.name.capacity(), 1)?;
        }
        output.allocation(sql.capacity(), 1)?;
        // NO_CLONE: The immutable SQL snapshot is shared with the command and
        // host request for exact reply correlation; no text payload is copied.
        let sql = Arc::clone(sql);
        Ok(QueryCreationOrigin::Draft {
            draft_revision: target.draft_revision,
            sql,
            source: target.source.take(),
        })
    })
}

pub(in crate::editing) fn review_saved(
    document: &mut Document,
    target: &mut SavedQueryTarget,
    destination: &str,
    clock: &dyn Clock,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_stored_query_copy").entered();
    preflight(document, target.document_revision, cancelled)?;
    let mut native = action_review_budget(document.edits.original.len())?;
    let mut syntax = syntax_budget();
    let image = capture(document, &mut native, cancelled)?;
    validate_saved(&image, target, &mut native, cancelled)?;
    let definition = read_saved_query_definition(
        &mut image.reader(),
        SCOPE,
        &target.name,
        &mut native,
        cancelled,
    )?;
    let actual = reconstruct_saved_read(definition, ParseLimits::default(), &mut syntax, cancelled)
        .map_err(NativeSavedReadError::from)?;
    check(cancelled)?;
    native.charge(Resource::Steps, 1)?;
    let Value::DateTime(captured_at) = ClockFunction::Now.apply(Some(clock))? else {
        return Err(Error::ParameterInput);
    };
    let plan = prepare_saved_query_creation(
        image,
        actual.query(),
        SavedQueryCreationOptions {
            destination,
            captured_at,
            limits: ParseLimits::default(),
        },
        &mut native,
        &mut syntax,
        cancelled,
    )?;
    issue(document, plan, &mut native, cancelled, || {
        let name = Presentation::new(cancelled).take_text(&mut target.name)?;
        Ok(QueryCreationOrigin::Saved(SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name,
        }))
    })
}

fn issue(
    document: &Document,
    plan: RecoverableNativeQueryCreationPlan,
    native: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    origin: impl FnOnce() -> Result<QueryCreationOrigin, Error>,
) -> Result<Response, Error> {
    let mut output = Presentation::new(cancelled);
    let id = issue_id()?;
    let intent = plan.intent();
    // NO_CLONE: The bounded response facts and reopen witness outlive the
    // consuming sealed native plan; command-owned input is transferred below.
    let destination = output.text(intent.name())?;
    let alias = output.text(intent.alias())?;
    let native_id = intent.native_query_id();
    let value = intent.value();
    let captured_at = intent.captured_at();
    native.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    native.charge_allocation(1, size_of::<QueryCreatedReopen>() as u64)?;
    native.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(QueryCreatedReopen {
        // NO_CLONE: The immutable reopen witness outlives this Document borrow
        // while the live source retains its own exact path owner.
        path: document.edits.path.clone(),
        expected: plan.after_shared_bytes(),
        document_revision: document.revision,
        // NO_CLONE: The read witness remains usable after SaveWorker consumes
        // the original; its bounded name cannot borrow the plan being moved.
        name: output.text(intent.name())?,
        native_id: intent.native_query_id(),
    });
    check(cancelled)?;
    let origin = origin()?;
    Ok(Response::QueryCreationReviewed {
        review: QueryCreationReview {
            id,
            document_revision: document.revision,
            origin,
            destination,
            native_id,
            alias,
            value,
            captured_at,
        },
        capability: ReviewedQueryCreation {
            state: Box::new(State {
                id,
                reopen,
                plan,
                // NO_CLONE: Document invalidation must also revoke this independent
                // capability on the save thread; the atomic handle shares no payload.
                valid: Arc::clone(&document.edits.valid),
            }),
        },
    })
}

fn validate_saved(
    image: &NativeImage,
    target: &SavedQueryTarget,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(cancelled)?;
    let catalog = image.catalog().catalog();
    work.charge(
        Resource::Steps,
        catalog.objects().len() as u64 + target.name.len() as u64,
    )?;
    let object = catalog
        .lookup(&target.name, Some(ObjectKind::Query), None)
        .map_err(InspectError::from)?
        .ok_or(Error::StaleSavedQuery)?;
    if object.id().as_u128() != target.object_id || object.name().as_str() != target.name {
        return Err(Error::StaleSavedQuery);
    }
    Ok(())
}
