//! Actual fixed-image retained pair setup shared by rename and DROP acceptance.
use super::super::{
    CompletionKind, OfflineWriteSession, SchemaChange, SessionState, VerifiedImage,
};
use super::assert_completion;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_ace::{RecoverableFieldDropPlan, RecoverableFieldRenamePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;

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
            assert!(plan.intent().is_retained_schema() || plan.intent().is_retained_typed());
            let encoded = plan.intent().encode(budget, &|| false).unwrap();
            let marker = if plan.intent().is_retained_typed() {
                b"TCFR0007"
            } else {
                b"TCFR0006"
            };
            assert!(encoded.starts_with(marker));
            encoded
        }
        FieldPlan::Drop(plan) => {
            assert!(
                plan.intent().is_retained()
                    || plan.intent().is_retained_typed()
                    || plan.intent().is_retained_non_tail()
            );
            let encoded = plan.intent().encode(budget, &|| false).unwrap();
            let marker = if plan.intent().is_retained_non_tail() {
                b"TCFD0005"
            } else if plan.intent().is_retained_typed() {
                b"TCFD0004"
            } else {
                b"TCFD0003"
            };
            assert!(encoded.starts_with(marker));
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

/// Check one real pending/committed or retired schema journal. Private session
/// ownership glue stays here; independent expected images come from the caller.
pub(in crate::editing::offline) fn assert_retained_schema_recovery(
    scope: CatalogScope,
    before: &[u8],
    after: &[u8],
    plan: FieldPlan,
    committed: bool,
    retired: bool,
    budget: &mut ResourceBudget,
) {
    let change = match &plan {
        FieldPlan::Rename(plan) => plan.changed().then_some(SchemaChange::ColumnRenamed),
        FieldPlan::Drop(_) => Some(SchemaChange::ColumnDropped),
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schema-recovery.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, before).unwrap();
    let owner = OfflineWriteSession::open(&path, scope, budget, &|| false).unwrap();
    let owner = retain_retained_pair(owner, &bundle, plan, committed, retired, budget);
    owner.release().unwrap();

    let owner = OfflineWriteSession::open(&path, scope, budget, &|| false).unwrap();
    let (owner, receipt) = if retired {
        owner
            .confirm_retired(&retired_namespace(directory.path()), budget, &|| false)
            .unwrap()
    } else {
        owner.recover_update(&bundle, budget, &|| false).unwrap()
    };
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        if committed { change } else { None }
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
    assert_completion(&receipt);
    if retired {
        assert_eq!(receipt.kind(), CompletionKind::Confirmed);
        assert_eq!(
            receipt.verified_image(),
            Some(if committed {
                VerifiedImage::Replacement
            } else {
                VerifiedImage::Original
            }),
        );
    } else {
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(
            receipt.outcome(),
            Some(if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }),
        );
    }
    owner.release().unwrap();
    let actual = fs::read(&path).unwrap();
    let expected = if committed { after } else { before };
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing image byte",
    );
}
