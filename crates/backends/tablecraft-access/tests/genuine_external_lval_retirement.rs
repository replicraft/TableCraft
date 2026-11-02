#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_external_lval_continuation as sources;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
#[ignore = "requires external continuation36 source qualification and matching normal fitting implementation"]
fn retained_external_retirement_saves_literal_null_and_empty_images_with_same_lease() {
    let mut saves = [0; 2];
    let mut empty_commits = 0;

    for case in sources::load_all(verify).into_iter().filter(|case| {
        matches!(
            case.recipe,
            "clear-single-blob"
                | "clear-single-memo"
                | "clear-single-both"
                | "empty-single-blob"
                | "clear-memo-chain"
        )
    }) {
        for typed in [true, false] {
            if !typed && sources::retirement_sql(&case).is_none() {
                continue;
            }
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                let updates = case
                    .groups
                    .iter()
                    .map(|group| {
                        group
                            .updates
                            .iter()
                            .map(|cell| {
                                CellUpdate::new(
                                    editor.snapshot().row_handle(cell.row).unwrap(),
                                    cell.replacement.clone(),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let columns = case
                    .groups
                    .iter()
                    .zip(&updates)
                    .map(|(group, updates)| ColumnUpdates {
                        column: group.column,
                        updates,
                    })
                    .collect::<Vec<_>>();
                editor
                    .plan_columns(&columns, &mut budget(), &|| false)
                    .unwrap()
            } else {
                let query = parse_query(
                    sources::retirement_sql(&case).unwrap(),
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            let matched = case.changed;
            assert_eq!(staged.matched_rows(), matched);
            assert_eq!(staged.changed_rows(), case.changed);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            locked(&path);
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(matched));
            assert_eq!(receipt.changed_rows(), Some(case.changed));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            locked(&path);
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.expected.as_slice());
            let session = if typed && case.recipe == "clear-single-both" {
                let staged = editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Memo",
                            updates: &[],
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (0, 0));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), Some(0));
                assert_eq!(receipt.changed_rows(), Some(0));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                locked(&path);
                empty_commits += 1;
                session
            } else {
                editor.into_session()
            };
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [20, 16]);
    assert_eq!(empty_commits, 4);
}

#[test]
#[ignore = "requires continuation36 qualification before matching terminal growth"]
fn retirement_keeps_old_handles_stale_and_the_completed_receipt() {
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.recipe == "clear-memo-chain")
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(4).unwrap();
        let staged = editor
            .plan_updates(
                "Memo",
                &[CellUpdate::new(stale, Value::Null)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), case.after);
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "Memo",
                &[CellUpdate::new(stale, Value::Null)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
