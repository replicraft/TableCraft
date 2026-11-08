//! Native UPDATE review retains its distinct sealed candidate.
use super::{QueryUpdateReview, ReviewedQueryUpdate};
use crate::{
    Error, InspectedQuery, ParameterInput, Response,
    controls::check,
    display::Presentation,
    document::Document,
    editing::{
        EditError,
        ids::issue_id,
        query_action::{
            QueryActionTarget,
            native::{self, Kind, Plan},
        },
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::RecoverableNativeEditPlan;
use tablecraft_query::UpdatePlan;

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryActionTarget,
    pub(in crate::editing) plan: UpdatePlan<RecoverableNativeEditPlan>,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(super) fn review(
    document: &mut Document,
    target: &InspectedQuery,
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let reviewed = native::review(document, target, inputs, output, cancelled, Kind::Update)?;
    let Plan::Update(plan) = reviewed.plan else {
        return Err(EditError::Unavailable.into());
    };

    let review = QueryUpdateReview {
        id: issue_id()?,
        document_revision: reviewed.target.document_revision,
        table: output.text(&reviewed.target.table)?,
        matched_rows: plan.matched_rows(),
        changed_rows: plan.backend().changed_rows(),
    };

    let capability = ReviewedQueryUpdate {
        state: Box::new(State {
            target: reviewed.target,
            plan,
            valid: reviewed.valid,
        }),
    };

    check(cancelled)?;
    Ok(Response::QueryUpdateReviewed { review, capability })
}
