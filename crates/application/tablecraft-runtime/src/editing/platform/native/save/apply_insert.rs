//! Fresh exact-source admission and ordinary one-row insertion under one lease.
use super::{
    RetainedOwner,
    outcomes::{finish, no_session, retain, retain_local},
};
use crate::{
    controls::{SCOPE, budget},
    editing::{ReviewedInsert, SaveFailureKind, SaveOutcome},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::{OfflineWriteSession, OpenError};
use tablecraft_ace::InspectError;
use tablecraft_db::OmittingInsertableTable;

pub(super) fn apply_insert(
    reviewed: ReviewedInsert,
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
    let mut work = budget();
    let session = match OfflineWriteSession::open(&reviewed.path, SCOPE, &mut work, &cancel) {
        Ok(session) => session,
        Err(error) => {
            let kind = match error {
                OpenError::Native(InspectError::Cancelled) => SaveFailureKind::Cancelled,
                OpenError::Native(
                    InspectError::Limit(_)
                    | InspectError::Allocation
                    | InspectError::StructureLimit,
                ) => SaveFailureKind::Resource,
                _ => SaveFailureKind::Open,
            };
            return SaveOutcome::Failed(no_session(kind));
        }
    };

    let inserter = match session.insert_table_if_original(
        &reviewed.target.table,
        &reviewed.original,
        &mut work,
        &cancel,
    ) {
        Ok(inserter) => inserter,
        Err(failure) => return retain(failure, retained),
    };
    let staged = match inserter.plan_insert_with_omissions(reviewed.values, &mut work, &cancel) {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };
    if staged.inserted_rows() != reviewed.plan.inserted_rows() {
        return retain_local(
            staged.into_session(),
            SaveFailureKind::InvalidState,
            retained,
        );
    }

    finish(staged.apply(&bundle, &mut work, &cancel), retained)
}
