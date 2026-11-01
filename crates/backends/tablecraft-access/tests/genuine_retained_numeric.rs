#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_retained_numeric::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(139);

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

fn rows(case: &Case, changed: bool) -> Vec<Vec<Value>> {
    (1..=case.rows)
        .map(|ordinal| {
            let id = i32::try_from(ordinal).unwrap();
            let marker = if changed && ordinal == 1 { 42 } else { id * 17 };
            vec![Value::Int32(id), case.key.clone(), Value::Int32(marker)]
        })
        .collect()
}

#[test]
#[ignore = "requires all pinned retained numeric originals and expected images"]
fn genuine_retained_numeric_sessions_save_sixteen_typed_and_sql_complete_images() {
    let cases = sources::load_all(verify);
    let mut saved = 0;
    for case in cases {
        for typed in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), rows(&case, false));
                let handle = editor.snapshot().row_handle(0).unwrap();
                editor
                    .plan_updates(
                        "Marker",
                        &[CellUpdate::new(handle, Value::Int32(42))],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let sql = format!("UPDATE {} SET Marker=42 WHERE [ID]=1", case.table);
                let query = parse_query(&sql, ParseLimits::default()).unwrap();
                session
                    .plan_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), rows(&case, true));
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saved += 1;
        }
    }
    assert_eq!(saved, 16);
}
