//! Actual retained target and complete independent ADD-column images.
use super::*;
use crate::editing::offline::{
    CompletionKind, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_add_column;

const SCOPE: CatalogScope = CatalogScope::new(797);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn review(source: &canonical_add_column::Source) -> RecoverableAddColumnPlan {
    let image = NativeImage::capture(
        &mut Cursor::new(&source.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_recoverable_add_int32_column("Items", "Extra", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), source.after);
    plan
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn reviewed_column_commits_exact_image_without_invented_row_counts() {
    let source = canonical_add_column::source("Extra");
    let reviewed = review(&source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("column.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.before).unwrap();
    let staged = open(&path)
        .stage_reviewed_add_column_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(staged.table(), "Items");
    assert_eq!(staged.column(), "Extra");
    assert!(!bundle.exists());
    assert_staged_read_state(&path, &source.before);
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
    assert_completion(&receipt);
    session.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), source.after);
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    source: &canonical_add_column::Source,
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, &source.before).unwrap();
    let plan = review(source);
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(intent.starts_with(b"TCAC0001"));
    let mut session = open(path);
    let slot = session.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
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
    session
}

#[test]
fn cold_recovery_reconstructs_original_tcac_and_reports_only_committed_schema_effect() {
    let source = canonical_add_column::source("Extra");
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recover.accdb");
        let bundle = directory.path().join("journal");
        retained_pair(&path, &bundle, &source, committed, false)
            .release()
            .unwrap();
        let (session, receipt) = open(&path)
            .recover_update(&bundle, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(
            receipt.outcome(),
            Some(if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            })
        );
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::ColumnAdded)
        );
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap().as_slice(),
            if committed {
                source.after.as_slice()
            } else {
                source.before.as_slice()
            }
        );
    }
}

#[test]
fn settled_confirmation_checks_the_exact_recorded_image_without_replaying_schema() {
    let source = canonical_add_column::source("Extra");
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirm.accdb");
        let bundle = directory.path().join("journal");
        let owner = retained_pair(&path, &bundle, &source, committed, true);
        owner.release().unwrap();
        let mut entries = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
            });
        let retired = entries.next().expect("one intact retired column journal");
        assert!(entries.next().is_none());
        let (session, receipt) = open(&path)
            .confirm_retired(&retired, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Confirmed);
        assert_eq!(
            receipt.verified_image(),
            Some(if committed {
                VerifiedImage::Replacement
            } else {
                VerifiedImage::Original
            })
        );
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::ColumnAdded)
        );
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap().as_slice(),
            if committed {
                source.after.as_slice()
            } else {
                source.before.as_slice()
            }
        );
    }
}

#[test]
fn changed_source_cancellation_and_separate_limits_retain_owner_before_setup() {
    for case in 0..4 {
        let source = canonical_add_column::source("Extra");
        let reviewed = review(&source);
        let mut current = source.before;
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &current).unwrap();
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_add_column_with_budgets(reviewed, &mut native, &mut proof, &|| {
                case == 3
            })
            .err()
            .expect("ADD column must refuse before journal setup");
        assert_eq!(failure.phase(), Phase::Planning);
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert!(failure.pair_state().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

#[test]
fn malformed_tcac_records_never_fall_back_to_another_action_decoder() {
    for raw in [b"TCAC".as_slice(), b"TCAC0001", b"TCAC0001\0", b"TCAC9999"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::AddColumnIntent(_))
        ));
    }
}

#[path = "default_zero_tests.rs"]
mod default_zero_tests;
