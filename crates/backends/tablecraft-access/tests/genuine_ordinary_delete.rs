#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, EditableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_ordinary_delete as sources;

const SCOPE: CatalogScope = CatalogScope::new(154);

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

#[test]
#[ignore = "requires all four pinned ordinary DELETE SOURCE images"]
fn typed_and_sql_ordinary_delete_commit_literal_images_and_preserve_empty_table_schema() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                let snapshot = EditableTable::snapshot(&editor);
                assert_eq!(
                    snapshot.rows(),
                    &[vec![
                        Value::Int32(23),
                        Value::Text("unchanged sentinel".into()),
                        Value::Binary(vec![199, 0, 255]),
                    ]]
                );
                let row = snapshot.row_handle(0).unwrap();
                DeletableTable::plan_deletes(editor, &[row], &mut budget(), &|| false).unwrap()
            } else {
                let query =
                    parse_query("DELETE FROM Sentinel WHERE [ID]=23", ParseLimits::default())
                        .unwrap();
                session
                    .plan_delete(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(1));
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
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
    assert_eq!(saves, 4);
}
