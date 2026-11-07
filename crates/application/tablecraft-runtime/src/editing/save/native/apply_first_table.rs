//! Reconstruct the schema under the same retained exclusive target lease.
use super::{
    RetainedOwner,
    outcomes::{finish, no_session, retain, retain_local},
};
use crate::{
    controls::{SCOPE, budget},
    editing::{ReviewedFirstTable, SaveFailureKind, SaveOutcome},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::{OfflineWriteSession, OpenError};
use tablecraft_ace::InspectError;

pub(super) fn apply_first_table(
    reviewed: ReviewedFirstTable,
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
    let session =
        match OfflineWriteSession::open(reviewed.reopen.source_path(), SCOPE, &mut work, &cancel) {
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
    let staged = match session.plan_first_table_if_original(
        reviewed.definition,
        &reviewed.original,
        &mut work,
        &cancel,
    ) {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };
    if staged.table() != reviewed.reopen.table_name()
        || staged.column_count() != reviewed.reopen.column_count()
    {
        return retain_local(
            staged.into_session(),
            SaveFailureKind::InvalidState,
            retained,
        );
    }

    finish(staged.apply(&bundle, &mut work, &cancel), retained)
}
