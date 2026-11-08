//! Reconstruct the schema under the same retained exclusive target lease.
use super::{
    RetainedOwner,
    outcomes::{finish, no_session, retain, retain_local},
};
use crate::{
    controls::{SCOPE, budget},
    editing::{ReviewedTableCreation, SaveFailureKind, SaveOutcome, TableCreationProfile},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::{
    OfflineWriteSession, OpenError, StagedAddTable, StagedFirstTable,
};
use tablecraft_ace::InspectError;

pub(super) fn apply_table_creation(
    reviewed: ReviewedTableCreation,
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
    let staged = match reviewed.profile {
        TableCreationProfile::First => session
            .plan_first_table_if_original(
                reviewed.definition,
                &reviewed.original,
                &mut work,
                &cancel,
            )
            .map(StagedCreation::First),
        TableCreationProfile::Added => session
            .plan_add_table_if_original(reviewed.definition, &reviewed.original, &mut work, &cancel)
            .map(StagedCreation::Added),
    };
    let staged = match staged {
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

    let result = match staged {
        StagedCreation::First(staged) => staged.apply(&bundle, &mut work, &cancel),
        StagedCreation::Added(staged) => staged.apply(&bundle, &mut work, &cancel),
    };
    finish(result, retained)
}

// Each variant retains the adapter's exact action capability and decoder family.
// Shared display checks do not authorize trying another action after refusal.
enum StagedCreation {
    First(StagedFirstTable),
    Added(StagedAddTable),
}

impl StagedCreation {
    fn table(&self) -> &str {
        match self {
            Self::First(staged) => staged.table(),
            Self::Added(staged) => staged.table(),
        }
    }

    fn column_count(&self) -> usize {
        match self {
            Self::First(staged) => staged.column_count(),
            Self::Added(staged) => staged.column_count(),
        }
    }

    fn into_session(self) -> OfflineWriteSession {
        match self {
            Self::First(staged) => staged.into_session(),
            Self::Added(staged) => staged.into_session(),
        }
    }
}
