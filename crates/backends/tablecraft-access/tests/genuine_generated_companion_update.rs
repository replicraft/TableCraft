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
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_generated_companion_update::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires twelve independently qualified generated-companion Score images"]
fn explicit_score_typed_and_sql_saves_refresh_rows_and_keep_last_defaults_deleted_bytes_and_lease()
{
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
            let orders = if typed { case.orders() } else { &[false] };
            for &reverse in orders {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("target.accdb");
                let journal = dir.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(244),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let editor = session
                    .edit_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), case.original_rows());
                let old_handle = editor.snapshot().row_handle(0).unwrap();
                let staged = if typed {
                    let cells: Vec<_> = case
                        .updates(reverse)
                        .into_iter()
                        .map(|(row, old, new)| {
                            assert_eq!(editor.snapshot().rows()[row][case.score_position()], old);
                            CellUpdate::new(editor.snapshot().row_handle(row).unwrap(), new)
                        })
                        .collect();
                    editor
                        .plan_columns(
                            &[ColumnUpdates {
                                column: "Score",
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
                    .edit_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.rows());
                let stale = CellUpdate::new(old_handle, Value::Int32(17));
                let failure = editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Score",
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
    }
    assert_eq!(saves, [18, 16]);
    assert_eq!(stale_handles, 34);
}

#[test]
#[ignore = "requires four independently qualified original generated-companion schemas"]
fn generated_id_and_exact_typed_carriers_refuse_before_namespace_publication_on_the_retained_lease()
{
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut generated = [0; 2];
    let mut wrong_carriers = 0;
    for case in cases.iter().filter(|case| case.recipe == Recipe::Changed) {
        for typed in [true, false] {
            for kind in 0..4 {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("target.accdb");
                fs::write(&path, &case.before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(245),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let failure = if typed {
                    let editor = session
                        .edit_table(case.table, &mut budget(), &|| false)
                        .unwrap();
                    let values = match kind {
                        0 => Some(Value::Int32(4)),
                        1 => Some(Value::Int32(1)),
                        2 => Some(Value::Null),
                        _ => None,
                    };
                    let cells: Vec<_> = values
                        .into_iter()
                        .map(|value| {
                            CellUpdate::new(editor.snapshot().row_handle(0).unwrap(), value)
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
                        .err()
                        .unwrap()
                } else {
                    let (value, id) = match kind {
                        0 => ("4", 1),
                        1 => ("1", 1),
                        2 => ("NULL", 1),
                        _ => ("4", 999),
                    };
                    let sql = format!("UPDATE [{}] SET [Id]={value} WHERE [Id]={id}", case.table);
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                        .err()
                        .unwrap()
                };
                if typed {
                    assert!(matches!(
                        failure.cause(),
                        Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                            EditError::Unsupported(WriteBlocker::Column)
                        )))
                    ));
                }
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.state(), SessionState::Ready);
                assert_eq!(failure.last_completed(), None);
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                failure.into_session().release().unwrap();
                assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
                generated[usize::from(!typed)] += 1;
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        fs::write(&path, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(246), &mut budget(), &|| false)
                .unwrap();
        let editor = session
            .edit_table(case.table, &mut budget(), &|| false)
            .unwrap();
        let cell = CellUpdate::new(editor.snapshot().row_handle(0).unwrap(), Value::Int64(17));
        let failure = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Score",
                    updates: &[cell],
                }],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Unsupported))
        ));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        wrong_carriers += 1;
    }
    assert_eq!(generated, [16, 16]);
    assert_eq!(wrong_carriers, 4);
}
