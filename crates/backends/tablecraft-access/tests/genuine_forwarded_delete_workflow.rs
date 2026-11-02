#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, DeletableTable, EditableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_forwarded_delete as sources;

const SCOPE: CatalogScope = CatalogScope::new(157);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn rows(marker: &str) -> Vec<Vec<Value>> {
    vec![vec![
        Value::Int32(23),
        Value::Text(marker.into()),
        Value::Binary(vec![199, 0, 255]),
    ]]
}

#[test]
#[ignore = "requires pinned EOF and forwarded DELETE SOURCE images"]
fn same_session_grow_then_delete_uses_fresh_typed_or_sql_rows_and_exact_images() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.original).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                let snapshot = EditableTable::snapshot(&editor);
                assert_eq!(snapshot.rows(), rows("unchanged sentinel"));
                let handle = snapshot.row_handle(0).unwrap();
                editor
                    .plan_updates(
                        "Marker",
                        &[CellUpdate::new(
                            handle,
                            Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
                        )],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(
                    "UPDATE Sentinel SET Marker='aaaaaaaaaaaaaaaaaaa' WHERE [ID]=23",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.original);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, growth) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(growth.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(growth.matched_rows(), Some(1));
            assert_eq!(growth.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(growth));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            saves += 1;

            // Reacquire the refreshed logical row after V2 growth. Never reuse
            // a handle from the snapshot which preceded forwarding/allocation.
            let editor = session
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            let snapshot = EditableTable::snapshot(&editor);
            assert_eq!(snapshot.rows(), rows("aaaaaaaaaaaaaaaaaaa"));
            let staged = if typed {
                let fresh = snapshot.row_handle(0).unwrap();
                DeletableTable::plan_deletes(editor, &[fresh], &mut budget(), &|| false).unwrap()
            } else {
                let query =
                    parse_query("DELETE FROM Sentinel WHERE [ID]=23", ParseLimits::default())
                        .unwrap();
                editor
                    .into_session()
                    .plan_delete(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            let (session, deleted) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(deleted.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(deleted.matched_rows(), Some(1));
            assert_eq!(deleted.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(deleted));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            assert!(EditableTable::snapshot(&editor).rows().is_empty());
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 8);
}
