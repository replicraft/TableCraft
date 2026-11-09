//! Reconstruct the original query request under the retained offline resource owner.
use super::{
    RetainedOwner,
    outcomes::{failed_open, finish, no_session, retain},
};
use crate::{
    controls::{SCOPE, action_persistence_budget, action_proof_budget, budget},
    editing::{ReviewedQuerySql, SaveFailureKind, SaveOutcome},
};
use std::{path::PathBuf, sync::atomic::Ordering};
use tablecraft_access::editing::offline::OfflineWriteSession;

pub(super) fn apply_query_sql(
    reviewed: ReviewedQuerySql,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let reviewed = *reviewed.state;
    if !reviewed.valid.load(Ordering::Acquire) {
        return SaveOutcome::Failed(no_session(SaveFailureKind::StaleDocument));
    }
    let cancel = || cancelled() || !reviewed.valid.load(Ordering::Acquire);
    let before = reviewed.plan.before_bytes().len();
    let after = reviewed.plan.after_bytes().len();
    let Ok(mut proof) = action_proof_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let Ok(mut persistence) = action_persistence_budget(before, after) else {
        return SaveOutcome::Failed(no_session(SaveFailureKind::Resource));
    };
    let mut native = budget();
    let session =
        match OfflineWriteSession::open(reviewed.reopen.source_path(), SCOPE, &mut native, &cancel)
        {
            Ok(session) => session,
            Err(error) => return failed_open(&error),
        };
    let staged = match session.stage_reviewed_query_expression_edits_with_budgets(
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
