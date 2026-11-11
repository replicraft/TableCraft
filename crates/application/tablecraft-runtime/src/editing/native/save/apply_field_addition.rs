//! Rebuild the exact field original intent under the retained offline owner.
use super::{
    RetainedOwner,
    outcomes::{failed_open, finish, no_session, retain},
};
use crate::editing::platform::field_addition::plan::Plan;
use crate::{
    controls::{SCOPE, action_persistence_budget, action_proof_budget, budget},
    editing::{
        AddedColumnReopen, ReviewedFieldAddition, ReviewedNumberColumn, SaveFailureKind,
        SaveOutcome,
    },
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_access::editing::offline::OfflineWriteSession;

pub(super) fn apply_number_column(
    reviewed: ReviewedNumberColumn,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let reviewed = *reviewed.state;
    apply_plan(
        reviewed.plan,
        reviewed.reopen,
        reviewed.valid,
        bundle,
        retained,
        cancelled,
    )
}

pub(super) fn apply_field_addition(
    reviewed: ReviewedFieldAddition,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let reviewed = *reviewed.state;
    apply_plan(
        reviewed.plan,
        reviewed.reopen,
        reviewed.valid,
        bundle,
        retained,
        cancelled,
    )
}

fn apply_plan(
    plan: Plan,
    reopen: Arc<AddedColumnReopen>,
    valid: Arc<AtomicBool>,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    if !valid.load(Ordering::Acquire) {
        return SaveOutcome::Failed(no_session(SaveFailureKind::StaleDocument));
    }
    let cancel = || cancelled() || !valid.load(Ordering::Acquire);
    let before = plan.before_bytes().len();
    let after = plan.after_bytes().len();
    let Ok(mut proof) = action_proof_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let Ok(mut persistence) = action_persistence_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let mut native = budget();
    let session = match OfflineWriteSession::open(reopen.source_path(), SCOPE, &mut native, &cancel)
    {
        Ok(session) => session,
        Err(error) => return failed_open(&error),
    };
    let staged = match plan {
        Plan::Declared(plan) => session
            .stage_reviewed_retained_declared_field_addition_with_budgets(
                *plan,
                &mut native,
                &mut proof,
                &cancel,
            ),
        Plan::RetainedTyped(plan) => session
            .stage_reviewed_retained_typed_field_addition_with_budgets(
                *plan,
                &mut native,
                &mut proof,
                &cancel,
            ),
        Plan::First(plan) => session.stage_reviewed_default_zero_add_column_with_budgets(
            plan,
            &mut native,
            &mut proof,
            &cancel,
        ),
        Plan::Repeated(plan) => session
            .stage_reviewed_repeated_default_zero_add_column_with_budgets(
                plan,
                &mut native,
                &mut proof,
                &cancel,
            ),
        Plan::Typed(plan) => session.stage_reviewed_field_addition_with_budgets(
            plan,
            &mut native,
            &mut proof,
            &cancel,
        ),
        Plan::AfterOff(plan) => session.stage_reviewed_field_addition_after_off_with_budgets(
            plan,
            &mut native,
            &mut proof,
            &cancel,
        ),
        Plan::AfterOffHistory(plan) => session
            .stage_reviewed_field_addition_after_off_history_with_budgets(
                plan,
                &mut native,
                &mut proof,
                &cancel,
            ),
        Plan::Current(plan) => session.stage_reviewed_current_field_addition_with_budgets(
            *plan,
            &mut native,
            &mut proof,
            &cancel,
        ),
    };
    let staged = match staged {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };
    finish(staged.apply(&bundle, &mut persistence, &cancel), retained)
}
