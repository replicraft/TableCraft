#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path, slice};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_inline_lval_update::{self as sources, Case};

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

fn statement(case: &Case) -> Option<&'static str> {
    match case.recipe {
        "noop" => Some("UPDATE InlineValues SET MemoValue='old' WHERE Id=1"),
        "memo-new" => Some("UPDATE InlineValues SET MemoValue='new' WHERE Id=1"),
        "raw-unicode" => Some("UPDATE InlineValues SET MemoValue='βγ' WHERE Id=1"),
        "memo-empty" => Some("UPDATE InlineValues SET MemoValue='' WHERE Id=1"),
        "memo-clear" => Some("UPDATE InlineValues SET MemoValue=NULL WHERE Id=1"),
        "blob-clear" => Some("UPDATE InlineValues SET BlobValue=NULL WHERE Id=1"),
        "both-null" => Some("UPDATE InlineValues SET MemoValue=NULL, BlobValue=NULL WHERE Id=1"),
        "empty-group" => Some("UPDATE InlineValues SET MemoValue='old' WHERE Id=-1"),
        _ => None,
    }
}

#[test]
#[ignore = "requires inline28 source qualification and matching normal fitting implementation"]
fn retained_inline_updates_save_typed_and_real_sql_images_without_coercion() {
    let mut saves = [0; 2];
    let mut empty_commits = 0;
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            if !typed && statement(&case).is_none() {
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
                    .edit_table("InlineValues", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                let handle = editor.snapshot().row_handle(0).unwrap();
                let updates = case
                    .groups
                    .iter()
                    .map(|group| {
                        group
                            .updates
                            .iter()
                            .map(|cell| CellUpdate::new(handle, cell.replacement.clone()))
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
                let query = parse_query(statement(&case).unwrap(), ParseLimits::default()).unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            let matched = usize::from(case.recipe != "empty-group");
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
                .edit_table("InlineValues", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), slice::from_ref(&case.expected));
            // Real same-lease empty planning rechecks the actual resulting long
            // carriers without constructing an unqualified second AFTER.
            let staged = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column: "MemoValue",
                        updates: &[],
                    }],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(staged.matched_rows(), 0);
            assert_eq!(staged.changed_rows(), 0);
            let (session, empty_receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(empty_receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(empty_receipt.matched_rows(), Some(0));
            assert_eq!(empty_receipt.changed_rows(), Some(0));
            assert_eq!(session.last_completed(), Some(empty_receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            locked(&path);
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
            empty_commits += 1;
        }
    }
    assert_eq!(saves, [26, 16]);
    assert_eq!(empty_commits, 42);
}

#[test]
#[ignore = "requires inline28 source qualification and matching normal fitting implementation"]
fn inline_capacity_external_payload_and_stale_handles_keep_the_prior_receipt_and_lease() {
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.recipe == "memo-new")
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("InlineValues", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "MemoValue",
                &[CellUpdate::new(stale, Value::Text("new".into()))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (mut session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), case.after);
        for external in [false, true] {
            let editor = session
                .edit_table("InlineValues", &mut budget(), &|| false)
                .unwrap();
            let row = editor.snapshot().row_handle(0).unwrap();
            let memo = [CellUpdate::new(row, Value::Null)];
            let blob = [CellUpdate::new(
                row,
                Value::Binary(vec![0; if external { 65 } else { 21 }]),
            )];
            let failure = editor
                .plan_columns(
                    &[
                        ColumnUpdates {
                            column: "MemoValue",
                            updates: &memo,
                        },
                        ColumnUpdates {
                            column: "BlobValue",
                            updates: &blob,
                        },
                    ],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            let expected = if external {
                WriteBlocker::Column
            } else {
                WriteBlocker::RowLayout
            };
            assert!(
                matches!(failure.cause(), Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(blocker)))) if *blocker == expected)
            );
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            locked(&path);
            session = failure.into_session();
        }
        let editor = session
            .edit_table("InlineValues", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "MemoValue",
                &[CellUpdate::new(stale, Value::Text("new".into()))],
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
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
    }
}
