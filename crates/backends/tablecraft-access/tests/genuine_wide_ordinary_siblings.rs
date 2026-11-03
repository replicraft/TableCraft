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
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_wide_ordinary_siblings::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires eighteen independently qualified wider-sibling images"]
fn typed_and_exact_carrier_sql_saves_refresh_handles_preserve_companions_and_retain_lease() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = [0; 2];
    let mut stale_refusals = 0;
    for case in cases {
        for typed in [true, false] {
            let sql = case.sql();
            if !typed && sql.is_none() {
                continue;
            }
            let orders = if typed { case.orders() } else { &[false] };
            for &reverse in orders {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let journal = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(254),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let editor = session
                    .edit_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), case.original_rows());
                let old = editor.snapshot().row_handle(0).unwrap();
                let staged = if typed {
                    let cells: Vec<_> = case
                        .updates(reverse)
                        .into_iter()
                        .map(|(row, expected, value)| {
                            assert_eq!(editor.snapshot().rows()[row][1], expected);
                            CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), value)
                        })
                        .collect();
                    editor
                        .plan_columns(
                            &[ColumnUpdates {
                                column: "KeyValue",
                                updates: &cells,
                            }],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let session = editor.into_session();
                    let query =
                        parse_query(sql.as_deref().unwrap(), ParseLimits::default()).unwrap();
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
                    .edit_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.rows());
                let stale = CellUpdate::new(old, case.number(0));
                let failure = editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "KeyValue",
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
                assert!(!journal.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves[usize::from(!typed)] += 1;
                stale_refusals += 1;
            }
        }
    }
    assert_eq!(saves, [28, 16]);
    assert_eq!(stale_refusals, 44);
}

#[test]
#[ignore = "requires four independently qualified original wider-sibling table contexts"]
fn exact_typed_carriers_and_primary_constraints_refuse_before_namespace_publication() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut refusals = 0;
    for case in cases.into_iter().filter(|case| case.recipe == Recipe::Zero) {
        let wrong = if case.table == "Int64Companions" {
            Value::Currency(Currency::from_scaled(0))
        } else {
            Value::Int64(0)
        };
        for (column, value) in [
            ("KeyValue", wrong),
            ("KeyValue", Value::Int32(0)),
            ("Id", Value::Null),
            ("Id", Value::Int32(2)),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(255), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
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
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            failure.into_session().release().unwrap();
            refusals += 1;
        }
    }
    assert_eq!(refusals, 16);
}
