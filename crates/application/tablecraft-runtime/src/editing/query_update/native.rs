//! Native lease-free review retains one checked plan and its source invalidation token.
use super::{QueryUpdateReview, QueryUpdateTarget, ReviewedQueryUpdate};
use crate::{
    Error, InspectedQuery, Response,
    controls::{SCOPE, check, comparison_budget, update_query_budget, update_review_budget},
    display::Presentation,
    document::Document,
    editing::{EditError, ids::issue_id},
};
use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};
use tablecraft_access::{NativeSource, editing::sql};
use tablecraft_ace::{InspectError, NativeImage, RecoverableNativeEditPlan};
use tablecraft_core::limits::Resource;
use tablecraft_query::UpdatePlan;
use tablecraft_sql::{ParseLimits, parse_query};

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryUpdateTarget,
    pub(in crate::editing) plan: UpdatePlan<RecoverableNativeEditPlan>,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(super) fn review(
    document: &mut Document,
    target: &InspectedQuery,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let mut query_work = update_query_budget();
    query_work.charge(Resource::Bytes, target.sql().len() as u64)?;
    query_work.charge(Resource::Steps, target.sql().len() as u64)?;
    let parsed = parse_query(target.sql(), ParseLimits::default())?;
    let table =
        sql::local_batch_update_table(parsed.query()).map_err(sql::RecoverableError::Shape)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }

    let native_pool = RefCell::new(update_review_budget(document.edits.original.len())?);
    let image = {
        let mut native = native_pool.borrow_mut();
        NativeImage::capture(&mut document.source, SCOPE, &mut native, cancelled)?
    };
    let source =
        NativeSource::new_with_shared_budget(&mut document.source, SCOPE, &native_pool, cancelled)?;
    let comparison_pool = RefCell::new(comparison_budget());
    let (_source, resolver) = source.with_deferred_database_collation_resolver(&comparison_pool)?;

    let plan = {
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        sql::plan_recoverable_batch_update_with_comparison(
            image,
            parsed.query(),
            &mut native,
            &mut query_work,
            resolver.default_collation(),
            &resolver,
            cancelled,
        )?
    };
    check(cancelled)?;

    let mut output = Presentation::new(cancelled);
    let review_table = output.text(table)?;
    let capability_table = output.text(table)?;
    let matched_rows = plan.matched_rows();
    let changed_rows = plan.backend().changed_rows();
    {
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        native.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
        native.charge_allocation(1, size_of::<State>() as u64)?;
    }
    let capability = ReviewedQueryUpdate {
        state: Box::new(State {
            target: QueryUpdateTarget {
                path: document.edits.path.clone(),
                document_revision: document.revision,
                table: capability_table,
            },
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    let review = QueryUpdateReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: review_table,
        matched_rows,
        changed_rows,
    };
    check(cancelled)?;
    Ok(Response::QueryUpdateReviewed { review, capability })
}
