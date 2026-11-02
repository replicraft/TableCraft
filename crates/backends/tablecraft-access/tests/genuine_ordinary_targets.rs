#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{EditError, IndexConstraintViolation, InspectError, RecoverableEditError};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_ordinary_targets::{self as sources, Recipe};

const SCOPE: CatalogScope = CatalogScope::new(125);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn source(header: u8, recipe: Recipe) -> sources::Case {
    let case = sources::source(header, recipe);
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    assert_eq!(digest(&case.before), case.before_digest);
    if let Some(after) = &case.after {
        assert_eq!(digest(after), case.after_digest.unwrap());
    }
    case
}

fn locked(path: &std::path::Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::ScoreSeventeen => "UPDATE Items SET Score=17 WHERE Id=1",
        Recipe::ScoreClear => "UPDATE Items SET Score=NULL WHERE Id=1",
        Recipe::ScoreNoop => "UPDATE Items SET Score=-1 WHERE Id=1",
        Recipe::ScoreEmpty => "UPDATE Items SET Score=17 WHERE Id=-1",
        Recipe::ManualIdSeven => "UPDATE Items SET Id=7 WHERE Id=1",
        _ => panic!("only the five frozen positive SQL contexts are used"),
    }
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_sessions_save_twenty_exact_images_through_typed_and_sql_services() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in sources::POSITIVES {
            let case = source(header, recipe);
            let after = case.after.as_ref().unwrap();
            for typed in [true, false] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), case.rows);
                    let updates = if case.empty {
                        Vec::new()
                    } else {
                        vec![CellUpdate::new(
                            editor.snapshot().row_handle(0).unwrap(),
                            sources::value(case.replacement),
                        )]
                    };
                    editor
                        .plan_updates(case.column, &updates, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(sql(recipe), ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (staged.matched_rows(), staged.changed_rows()),
                    (usize::from(!case.empty), case.changed_rows)
                );
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.final_rows);
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 20);
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_duplicate_requests_and_actual_stale_handles_preserve_files_and_receipts() {
    for header in [5, 6] {
        let case = source(header, Recipe::DuplicateId);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let updates = [CellUpdate::new(
            editor.snapshot().row_handle(0).unwrap(),
            sources::value(case.replacement),
        )];
        let failure = editor
            .plan_updates("Id", &updates, &mut budget(), &|| false)
            .err()
            .expect("the duplicate primary key cannot stage a session update");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                ))
            )))
        ));
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        locked(&path);
        failure.into_session().release().unwrap();

        let case = source(header, Recipe::ScoreSeventeen);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, sources::value(Some(17)))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, sources::value(Some(18)))],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("a prior snapshot cannot supply the current row handle");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(
            fs::read(&path).unwrap().as_slice(),
            case.after.as_ref().unwrap().as_slice()
        );
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}
