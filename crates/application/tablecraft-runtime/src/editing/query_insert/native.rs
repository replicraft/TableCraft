//! Native INSERT review retains its distinct sealed candidate.
use super::{QueryInsertReview, ReviewedQueryInsert};
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
use tablecraft_access::editing::journal::NativeJournalInsertPlan;
use tablecraft_expressions::functions::dates::clock::Clock;

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryActionTarget,
    pub(in crate::editing) plan: NativeJournalInsertPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(super) fn review(
    document: &mut Document,
    target: &InspectedQuery,
    inputs: &[ParameterInput],
    clock: &dyn Clock,
    output: &mut Presentation<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let reviewed = native::review(
        document,
        target,
        inputs,
        output,
        cancelled,
        Kind::Insert(clock),
    )?;
    let Plan::Insert(plan) = reviewed.plan else {
        return Err(EditError::Unavailable.into());
    };

    let review = QueryInsertReview {
        id: issue_id()?,
        document_revision: reviewed.target.document_revision,
        table: output.text(&reviewed.target.table)?,
        inserted_rows: plan.inserted_rows(),
    };

    let capability = ReviewedQueryInsert {
        state: Box::new(State {
            target: reviewed.target,
            plan,
            valid: reviewed.valid,
        }),
    };

    check(cancelled)?;
    Ok(Response::QueryInsertReviewed { review, capability })
}
