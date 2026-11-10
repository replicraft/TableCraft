//! Actual fixed-image retained pair setup shared by rename and DROP acceptance.
use super::super::{OfflineWriteSession, SessionState};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_ace::{RecoverableFieldDropPlan, RecoverableFieldRenamePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};

pub(in crate::editing::offline) enum FieldPlan {
    Rename(RecoverableFieldRenamePlan),
    Drop(RecoverableFieldDropPlan),
}

/// Seed an actual V1 native pair; no caller AFTER or invented report is accepted.
pub(in crate::editing::offline) fn retain_pair(
    mut owner: OfflineWriteSession,
    bundle: &Path,
    plan: FieldPlan,
    committed: bool,
    retired: bool,
    budget: &mut ResourceBudget,
) -> OfflineWriteSession {
    let intent = match &plan {
        FieldPlan::Rename(plan) => plan.intent().encode(budget, &|| false).unwrap(),
        FieldPlan::Drop(plan) => plan.intent().encode(budget, &|| false).unwrap(),
    };
    let header: &[u8] = match &plan {
        FieldPlan::Rename(_) => b"TCFR0001",
        FieldPlan::Drop(_) => b"TCFD0001",
    };
    assert!(intent.starts_with(header));
    let slot = owner.slot_mut().unwrap();
    let mut pair =
        OfflineJournalPair::create(slot.target.take().unwrap(), bundle, intent, budget, &|| {
            false
        })
        .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = match plan {
            FieldPlan::Rename(plan) => plan.prepare(&mut target, &mut journal, budget, &|| false),
            FieldPlan::Drop(plan) => plan.prepare(&mut target, &mut journal, budget, &|| false),
        }
        .unwrap();
        if committed {
            prepared.commit(budget, &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(budget, &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner
}

pub(in crate::editing::offline) fn retired_namespace(directory: &Path) -> PathBuf {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
        });
    let path = entries.next().expect("retained retired journal namespace");
    assert!(entries.next().is_none());
    path
}

/// Seed only the retained schema protocol; never relabel a legacy V1 plan.
pub(in crate::editing::offline) fn retain_retained_pair(
    mut owner: OfflineWriteSession,
    bundle: &Path,
    plan: FieldPlan,
    committed: bool,
    retired: bool,
    budget: &mut ResourceBudget,
) -> OfflineWriteSession {
    let intent = match &plan {
        FieldPlan::Rename(plan) => {
            assert!(plan.intent().is_retained_schema());
            let encoded = plan.intent().encode(budget, &|| false).unwrap();
            assert!(encoded.starts_with(b"TCFR0006"));
            encoded
        }
        FieldPlan::Drop(plan) => {
            assert!(plan.intent().is_retained());
            let encoded = plan.intent().encode(budget, &|| false).unwrap();
            assert!(encoded.starts_with(b"TCFD0003"));
            encoded
        }
    };
    let slot = owner.slot_mut().unwrap();
    let mut pair =
        OfflineJournalPair::create(slot.target.take().unwrap(), bundle, intent, budget, &|| {
            false
        })
        .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        let prepared = match plan {
            FieldPlan::Rename(plan) => {
                plan.prepare_retained(&mut target, &mut journal, budget, &|| false)
            }
            FieldPlan::Drop(plan) => {
                plan.prepare_retained(&mut target, &mut journal, budget, &|| false)
            }
        }
        .unwrap();
        if committed {
            prepared.commit(budget, &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(budget, &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner
}
