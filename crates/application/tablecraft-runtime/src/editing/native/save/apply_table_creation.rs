//! Reconstruct the schema under the same retained exclusive target lease.
use super::{
    RetainedOwner,
    outcomes::{finish, no_session, retain, retain_local},
};
use crate::{
    controls::{SCOPE, action_persistence_budget, action_proof_budget, budget},
    editing::{
        ReviewedTableCreation, SaveFailureKind, SaveOutcome,
        platform::table_creation::{Request, TableCreationProfile},
    },
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::{
    OfflineWriteSession, OpenError, StagedAddTable, StagedFirstTable, StagedPopulatedAddTable,
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

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
    let staged = match reviewed.request {
        Request::Empty {
            original,
            definition,
            profile,
        } => match profile {
            TableCreationProfile::First => session
                .plan_first_table_if_original(definition, &original, &mut work, &cancel)
                .map(StagedCreation::First),
            TableCreationProfile::Added | TableCreationProfile::AddedNative => session
                .plan_add_table_if_original(definition, &original, &mut work, &cancel)
                .map(StagedCreation::Added),
        },
        Request::Populated(plan) => {
            let before = plan.before_bytes().len();
            let after = plan.after_bytes().len();
            let (Ok(mut proof), Ok(persistence)) = (
                action_proof_budget(before, after),
                action_persistence_budget(before, after),
            ) else {
                return retain_local(session, SaveFailureKind::Resource, retained);
            };
            session
                .stage_reviewed_populated_add_table_with_budgets(
                    plan, &mut work, &mut proof, &cancel,
                )
                .map(|staged| StagedCreation::Populated {
                    staged,
                    persistence,
                })
        }
    };
    let staged = match staged {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };
    if staged.table() != reviewed.reopen.table_name()
        || staged.column_count() != reviewed.reopen.column_count()
        || staged.inserted_rows() != reviewed.reopen.inserted_rows()
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
        StagedCreation::Populated {
            staged,
            mut persistence,
        } => staged.apply(&bundle, &mut persistence, &cancel),
    };
    finish(result, retained)
}

// Each variant retains the adapter's exact action capability and decoder family.
// Shared display checks do not authorize trying another action after refusal.
enum StagedCreation {
    First(StagedFirstTable),
    Added(StagedAddTable),
    Populated {
        staged: StagedPopulatedAddTable,
        persistence: ResourceBudget,
    },
}

impl StagedCreation {
    fn table(&self) -> &str {
        match self {
            Self::First(staged) => staged.table(),
            Self::Added(staged) => staged.table(),
            Self::Populated { staged, .. } => staged.table(),
        }
    }

    fn column_count(&self) -> usize {
        match self {
            Self::First(staged) => staged.column_count(),
            Self::Added(staged) => staged.column_count(),
            Self::Populated { staged, .. } => staged.column_count(),
        }
    }

    fn inserted_rows(&self) -> Option<usize> {
        match self {
            Self::Populated { staged, .. } => Some(staged.inserted_rows()),
            Self::First(_) | Self::Added(_) => None,
        }
    }

    fn into_session(self) -> OfflineWriteSession {
        match self {
            Self::First(staged) => staged.into_session(),
            Self::Added(staged) => staged.into_session(),
            Self::Populated { staged, .. } => staged.into_session(),
        }
    }
}
