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
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_retained_binary_marker::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn marker_typed_and_sql_saves_preserve_indexes_refresh_rows_and_retain_the_same_lease() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = [0; 2];
    for case in &cases {
        for typed in [true, false] {
            let orders: &[bool] = if typed && case.recipe == Recipe::Batch {
                &[false, true]
            } else {
                &[false]
            };
            for &reverse in orders {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("target.accdb");
                let journal = dir.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(239),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("BinaryKeys", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    let cells: Vec<_> = case
                        .updates(reverse)
                        .into_iter()
                        .map(|(row, old, new)| {
                            assert_eq!(editor.snapshot().rows()[row][2], old);
                            CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), new)
                        })
                        .collect();
                    editor
                        .plan_columns(
                            &[ColumnUpdates {
                                column: "Marker",
                                updates: &cells,
                            }],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
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
                let editor = session
                    .edit_table("BinaryKeys", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.rows());
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
                saves[usize::from(!typed)] += 1;
            }
        }
    }
    assert_eq!(saves, [10, 8]);
}

#[test]
#[ignore = "requires independently qualified original Binary schemas"]
fn marker_typed_wrong_carrier_and_foreign_snapshot_refuse_before_publication() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    for case in cases.iter().filter(|case| case.recipe == Recipe::Batch) {
        {
            let column = "Marker";
            let value = Value::Int64(29);
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(240), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            let update = CellUpdate::new(editor.snapshot().row_handle(0).unwrap(), value);
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column,
                        updates: &[update],
                    }],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), None);
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Native(NativeEditError::Unsupported))
            ));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        fs::write(&path, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(241), &mut budget(), &|| false)
                .unwrap();
        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let old = editor.snapshot().row_handle(0).unwrap();
        let session = editor.into_session();
        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let update = CellUpdate::new(old, Value::Int32(29));
        let failure = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Marker",
                    updates: &[update],
                }],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                tablecraft_db::SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
