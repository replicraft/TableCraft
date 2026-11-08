//! Persist only the reviewed native UPDATE, freshly requalified under one lease.
use super::{
    RetainedOwner,
    outcomes::{failed_open, finish, no_session, retain},
};
use crate::{
    controls::{SCOPE, budget, update_persistence_budget, update_proof_budget},
    editing::{ReviewedQueryUpdate, SaveFailureKind, SaveOutcome},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::OfflineWriteSession;

pub(super) fn apply_query_update(
    reviewed: ReviewedQueryUpdate,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let reviewed = *reviewed.state;
    if !reviewed.valid.load(Ordering::Acquire) {
        return SaveOutcome::Failed(no_session(SaveFailureKind::StaleDocument));
    }
    let valid = Arc::clone(&reviewed.valid);
    let cancel = || cancelled() || !valid.load(Ordering::Acquire);
    let before = reviewed.plan.backend().before_bytes().len();
    let after = reviewed.plan.backend().after_bytes().len();
    let Ok(mut proof) = update_proof_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let Ok(mut persistence) = update_persistence_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let mut native = budget();
    let session = match OfflineWriteSession::open(&reviewed.path, SCOPE, &mut native, &cancel) {
        Ok(session) => session,
        Err(error) => return failed_open(&error),
    };
    let staged = match session.stage_reviewed_update_with_budgets(
        reviewed.plan,
        &mut native,
        &mut proof,
        &cancel,
    ) {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };

    finish(staged.apply(&bundle, &mut persistence, &cancel), retained)
}
