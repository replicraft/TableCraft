#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
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
use tablecraft_testkit::fixtures::access::genuine_text_storage_carriers as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires separately qualified Text Source and fitting writer"]
fn general_text_typed_and_sql_updates_keep_exact_files_and_stale_handle_receipts() {
    let cases = sources::load_all(|bytes, expected| {
        assert_eq!(
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut saves = [0; 2];
    for case in cases {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(229), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            assert_eq!(editor.snapshot().rows(), case.rows(false));
            let old = editor.snapshot().row_handle(1).unwrap();
            let staged = if typed {
                let cells = [CellUpdate::new(old, Value::Text(case.value.into()))];
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
                let query = parse_query(
                    &format!(
                        "UPDATE [{}] SET [KeyValue]='{}' WHERE [ID]=2",
                        case.table, case.value
                    ),
                    ParseLimits::default(),
                )
                .unwrap();
                editor
                    .into_session()
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (Some(1), Some(1))
            );
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.rows(true));
            assert_eq!(editor.snapshot().resolve(old), Err(SnapshotError::StaleRow));
            let cells = [CellUpdate::new(old, Value::Null)];
            let failure = editor
                .plan_columns(
                    &[ColumnUpdates {
                        column: "KeyValue",
                        updates: &cells,
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
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [16, 16]);
}
