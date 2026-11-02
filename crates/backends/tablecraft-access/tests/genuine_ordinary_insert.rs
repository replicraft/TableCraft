#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(159);

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

fn expected_rows() -> Vec<Vec<Value>> {
    vec![
        vec![
            Value::Int32(23),
            Value::Text("unchanged sentinel".into()),
            Value::Binary(vec![199, 0, 255]),
        ],
        vec![Value::Int32(24), Value::Text("new".into()), Value::Null],
    ]
}

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn typed_and_sql_insert_then_same_session_noop_preserve_literal_images_and_counts() {
    let cases = sources::load_all(verify);
    let mut inserted = 0;
    let mut followed = 0;
    for case in cases {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let inserter = session
                    .insert_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(inserter.name(), "Sentinel");
                assert_eq!(
                    inserter
                        .columns()
                        .iter()
                        .map(|column| column.name())
                        .collect::<Vec<_>>(),
                    ["ID", "Marker", "Bytes"],
                );
                assert!(!inserter.requires_field_list());
                inserter
                    .plan_insert(sources::values(), &mut budget(), &|| false)
                    .unwrap()
            } else {
                // Written field order differs from storage order; persist the
                // resolved complete carriers, not SQL text or omission state.
                let query = parse_query(
                    "INSERT INTO Sentinel (Bytes, Marker, [ID]) VALUES (NULL, 'new', 24)",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            inserted += 1;

            // A fresh snapshot on the SAME lease sees the new row and issues a
            // real handle. A follow-on noop uses the unchanged literal AFTER.
            let editor = session
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            let snapshot = EditableTable::snapshot(&editor);
            assert_eq!(snapshot.rows(), expected_rows());
            let handle = snapshot.row_handle(1).unwrap();
            let staged = if typed {
                editor
                    .plan_updates(
                        "Marker",
                        &[CellUpdate::new(handle, Value::Text("new".into()))],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(
                    "UPDATE Sentinel SET Marker='new' WHERE [ID]=24",
                    ParseLimits::default(),
                )
                .unwrap();
                editor
                    .into_session()
                    .plan_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 0));
            let (session, noop) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(noop.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(noop.matched_rows(), Some(1));
            assert_eq!(noop.changed_rows(), Some(0));
            assert_eq!(session.last_completed(), Some(noop));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            followed += 1;
        }
    }
    assert_eq!((inserted, followed), (4, 4));
}

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn incomplete_rows_and_nonnull_ole_refuse_before_bundle_with_retained_session() {
    for case in sources::load_all(verify) {
        for values in [
            vec![Value::Int32(24), Value::Text("new".into())],
            vec![
                Value::Int32(24),
                Value::Text("new".into()),
                Value::Binary(vec![1]),
            ],
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let inserter = session
                .insert_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            let failure = inserter
                .plan_insert(values, &mut budget(), &|| false)
                .err()
                .unwrap();
            assert!(failure.invalid_input());
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
        for sql in [
            "INSERT INTO Sentinel ([ID], Marker) VALUES (24, 'new')",
            "INSERT INTO Sentinel ([ID], Marker, Marker) VALUES (24, 'new', 'new')",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let failure = session
                .plan_insert(query.query(), &mut budget(), &|| false)
                .err()
                .unwrap();
            assert!(failure.invalid_input());
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}
