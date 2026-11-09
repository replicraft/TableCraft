//! Lease-free existing-query review retains the native original request and scope.
use crate::{
    Error, Response, SavedQueryTarget,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, QueryExpressionReopen, QueryExpressionReview, ReviewedQueryExpression,
        platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{InspectError, NativeImage, RecoverableNativeQueryExpressionPlan};
use tablecraft_core::limits::Resource;
use tablecraft_model::ObjectKind;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<QueryExpressionReopen>,
    pub(in crate::editing) plan: RecoverableNativeQueryExpressionPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    target: &SavedQueryTarget,
    expected: &str,
    replacement: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_query_expression").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut work = budget();
    check(cancelled)?;
    let image = NativeImage::capture_shared(
        // NativeImage owns the checked source while Document keeps using it; this shares
        // immutable bytes instead of copying the database image.
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
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
    let plan = image.plan_recoverable_query_literal_expression(
        &target.name,
        expected,
        replacement,
        &mut work,
        cancelled,
    )?;
    let expected_bytes = super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = QueryExpressionReview {
        id: issue_id()?,
        target: SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: output.text(plan.query())?,
        },
        native_id: plan.native_id(),
        expected: output.text(expected)?,
        replacement: output.text(replacement)?,
        changed: plan.changed(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<QueryExpressionReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(QueryExpressionReopen {
        // The capability or reopen witness outlives this borrowed Document call and owns its
        // source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected: expected_bytes,
        target: SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: output.text(plan.query())?,
        },
        native_id: plan.native_id(),
    });
    let capability = ReviewedQueryExpression {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::QueryExpressionReviewed { review, capability })
}
