//! Persist a distinct sealed action through shared retained save ownership.
use super::{
    RetainedOwner,
    outcomes::{failed_open, finish, no_session, retain},
};
use crate::{
    controls::{SCOPE, action_persistence_budget, action_proof_budget, budget},
    editing::{
        SaveFailureKind, SaveOutcome,
        query_action::{QueryActionTarget, native::Plan},
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

pub(super) fn apply_query_action(
    target: QueryActionTarget,
    plan: Plan,
    valid: Arc<AtomicBool>,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    if !valid.load(Ordering::Acquire) {
        return SaveOutcome::Failed(no_session(SaveFailureKind::StaleDocument));
    }
    let cancel = || cancelled() || !valid.load(Ordering::Acquire);
    let (before, after) = match &plan {
        Plan::Update(plan) => (
            plan.backend().before_bytes().len(),
            plan.backend().after_bytes().len(),
        ),
        Plan::Delete(plan) => (
            plan.backend().before_bytes().len(),
            plan.backend().after_bytes().len(),
        ),
        Plan::Insert(plan) => (plan.before_bytes().len(), plan.after_bytes().len()),
    };
    let Ok(mut proof) = action_proof_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let Ok(mut persistence) = action_persistence_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let mut native = budget();
    let session = match OfflineWriteSession::open(&target.path, SCOPE, &mut native, &cancel) {
        Ok(session) => session,
        Err(error) => return failed_open(&error),
    };
    let result = match plan {
        Plan::Update(plan) => {
            match session.stage_reviewed_update_with_budgets(plan, &mut native, &mut proof, &cancel)
            {
                Ok(staged) => staged.apply(&bundle, &mut persistence, &cancel),
                Err(failure) => return retain(failure, retained),
            }
        }
        Plan::Delete(plan) => {
            match session.stage_reviewed_delete_with_budgets(plan, &mut native, &mut proof, &cancel)
            {
                Ok(staged) => staged.apply(&bundle, &mut persistence, &cancel),
                Err(failure) => return retain(failure, retained),
            }
        }
        Plan::Insert(plan) => {
            match session.stage_reviewed_insert_with_budgets(plan, &mut native, &mut proof, &cancel)
            {
                Ok(staged) => staged.apply(&bundle, &mut persistence, &cancel),
                Err(failure) => return retain(failure, retained),
            }
        }
    };
    finish(result, retained)
}
