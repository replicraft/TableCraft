//! Native DELETE review retains its distinct sealed candidate.
use crate::QueryExpressionFormats;
use crate::editing::{QueryDeleteReview, ReviewedQueryDelete};
use crate::{
    Error, InspectedQuery, ParameterInput, Response,
    controls::check,
    display::Presentation,
    document::Document,
    editing::{
        EditError,
        platform::ids::issue_id,
        platform::query_action::{self, Kind, Plan},
        query_action::QueryActionTarget,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::RecoverableNativeDeletePlan;
use tablecraft_query::DeletePlan;

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryActionTarget,
    pub(in crate::editing) plan: DeletePlan<RecoverableNativeDeletePlan>,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    target: &InspectedQuery,
    inputs: &[ParameterInput],
    formats: QueryExpressionFormats,
    output: &mut Presentation<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let reviewed = query_action::review(
        document,
        target,
        inputs,
        output,
        cancelled,
        Kind::Delete(formats),
    )?;
    let Plan::Delete(plan) = reviewed.plan else {
        return Err(EditError::Unavailable.into());
    };

    let review = QueryDeleteReview {
        id: issue_id()?,
        document_revision: reviewed.target.document_revision,
        table: output.text(&reviewed.target.table)?,
        matched_rows: plan.matched_rows(),
        deleted_rows: plan.backend().deleted_rows(),
    };

    let capability = ReviewedQueryDelete {
        state: Box::new(State {
            target: reviewed.target,
            plan,
            valid: reviewed.valid,
        }),
    };

    check(cancelled)?;
    Ok(Response::QueryDeleteReviewed { review, capability })
}
