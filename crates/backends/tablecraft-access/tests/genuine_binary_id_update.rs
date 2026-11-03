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
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_binary_id_update::{self as source};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires four independently qualified Binary Id images"]
fn id_typed_sql_saves_refresh_handles_keep_receipts_and_preserve_binary_bytes() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = [0; 2];
    let mut stale_handles = 0;
    for case in &cases {
        for typed in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let journal = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(244), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            assert_eq!(
                editor.snapshot().rows(),
                tablecraft_testkit::fixtures::access::genuine_binary_indexed::original_rows()
            );
            let old_handle = editor.snapshot().row_handle(0).unwrap();
            let staged = if typed {
                let cells: Vec<_> = case
                    .updates()
                    .into_iter()
                    .map(|(row, old, new)| {
                        assert_eq!(editor.snapshot().rows()[row][0], old);
                        CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), new)
                    })
                    .collect();
                editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Id",
                            updates: &cells,
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                // SQL resolves its own original snapshot on the same lease.
                let session = editor.into_session();
                let query = parse_query(case.sql(), ParseLimits::default()).unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!(
                (staged.matched_rows(), staged.changed_rows()),
                (case.matched(), case.changed())
            );
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(case.matched()));
            assert_eq!(receipt.changed_rows(), Some(case.changed()));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!journal.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            case.assert_preserved();
            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.rows());
            let stale = CellUpdate::new(old_handle, Value::Int32(17));
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column: "Id",
                        updates: &[stale],
                    }],
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
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
            stale_handles += 1;
        }
    }
    assert_eq!(saves, [6, 6]);
    assert_eq!(stale_handles, 12);
}

#[test]
#[ignore = "requires two independently qualified original Binary schemas"]
fn binary_targets_required_and_duplicate_primary_refuse_before_publication() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut refusals = 0;
    for case in cases
        .iter()
        .filter(|case| case.recipe == source::Recipe::Changed)
    {
        for (column, value) in [
            ("KeyValue", Value::Int32(14)),
            ("Id", Value::Null),
            ("Id", Value::Int32(2)),
            ("Id", Value::Int64(14)),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(246), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            let cell = CellUpdate::new(editor.snapshot().row_handle(0).unwrap(), value);
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column,
                        updates: &[cell],
                    }],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            failure.into_session().release().unwrap();
            refusals += 1;
        }
        for sql in [
            "UPDATE BinaryKeys SET KeyValue=14 WHERE Id=1",
            "UPDATE BinaryKeys SET Id=NULL WHERE Id=1",
            "UPDATE BinaryKeys SET Id=2 WHERE Id=1",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(247), &mut budget(), &|| false)
                    .unwrap();
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let failure = session
                .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                .err()
                .unwrap();
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            failure.into_session().release().unwrap();
            refusals += 1;
        }
    }
    assert_eq!(refusals, 14);
}
