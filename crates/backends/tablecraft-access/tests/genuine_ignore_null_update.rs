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
use tablecraft_testkit::fixtures::access::genuine_ignore_null_update::{
    self as source, Case, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    source::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires held genuine IgnoreNULL original/AFTER images"]
fn typed_and_sql_omission_saves_keep_lease_receipts_stale_handles_and_literal_images() {
    let mut saves = [0; 2];
    let mut stale_handles = 0;
    for case in cases() {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let journal_path = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(265), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table("Single", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            assert_eq!(editor.snapshot().rows(), source::original_rows("Single"));
            let old_handle = editor.snapshot().row_handle(0).unwrap();
            let staged = if typed {
                let cells: Vec<_> = case
                    .updates()
                    .into_iter()
                    .map(|(row, before, after)| {
                        assert_eq!(editor.snapshot().rows()[row][1], before);
                        CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), after)
                    })
                    .collect();
                editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Left",
                            updates: &cells,
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(case.sql(), ParseLimits::default()).unwrap();
                editor
                    .into_session()
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!(
                (staged.matched_rows(), staged.changed_rows()),
                (case.matched(), case.changed())
            );
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal_path.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged
                .apply(&journal_path, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(case.matched()));
            assert_eq!(receipt.changed_rows(), Some(case.changed()));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!journal_path.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table("Single", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.rows());
            let stale = CellUpdate::new(old_handle, Value::Int32(30));
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column: "Left",
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
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
            stale_handles += 1;
        }
    }
    assert_eq!(saves, [12, 12]);
    assert_eq!(stale_handles, 24);
}

#[test]
#[ignore = "requires genuine IgnoreNULL Required/Unique and Single originals"]
fn typed_family_orders_and_sql_constraints_refuse_before_publication() {
    let mut typed_refusals = 0;
    let mut sql_refusals = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        for refusal in source::REFUSALS {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(266), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table(refusal.table(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                editor.snapshot().rows(),
                source::original_rows(refusal.table())
            );
            let cells: Vec<_> = refusal
                .updates()
                .into_iter()
                .map(|(row, before, after)| {
                    assert_eq!(editor.snapshot().rows()[row][1], before);
                    CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), after)
                })
                .collect();
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column: "Left",
                        updates: &cells,
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
            typed_refusals += 1;

            // SQL does not claim caller-controlled request ordering. Only the
            // two distinct scalar constraint statements are repeated here.
            if let Some(sql) = refusal.sql() {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                fs::write(&path, &case.before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(267),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let query = parse_query(sql, ParseLimits::default()).unwrap();
                let failure = session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
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
                sql_refusals += 1;
            }
        }
    }
    assert_eq!((typed_refusals, sql_refusals), (12, 4));
}
