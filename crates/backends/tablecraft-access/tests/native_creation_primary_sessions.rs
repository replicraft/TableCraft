#![cfg(all(
    feature = "ace16-create",
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::fs;
use tablecraft_access::{
    creation::CreationDestination,
    editing::{
        journal,
        offline::{Cause, OfflineWriteSession, Phase},
        sql::RecoverableInsertError,
    },
};
use tablecraft_ace::{
    CreateColumn, EditError, IndexConstraintViolation, InspectError, RecoverableEditError,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_creation_primary as sources;

const SCOPE: CatalogScope = CatalogScope::new(222);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn index_failure(cause: &Cause, expected: IndexConstraintViolation) {
    let error = match cause {
        Cause::Editing(error) => error,
        Cause::InsertPlanning(RecoverableInsertError::Plan(InsertError::Backend(error))) => error,
        _ => panic!("expected the actual native index-constraint boundary"),
    };
    assert!(
        matches!(error, journal::Error::Plan(RecoverableEditError::Edit(
        EditError::Inspect(InspectError::IndexConstraint(actual))
    )) if *actual == expected)
    );
}

#[test]
fn newly_published_manual_primary_matches_literal_insert_and_retains_lease_after_key_refusals() {
    let mut saves = 0;
    let mut refusals = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        let Some((after, rows)) = sources::inserted(&source) else {
            continue;
        };
        for typed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("created.accdb");
            let bundle = directory.path().join("journal");
            let columns = [
                CreateColumn::new("Id", ScalarType::Int32)
                    .with_primary_key("PrimaryId")
                    .unwrap(),
                CreateColumn::new("Score", ScalarType::Int32),
            ];
            let original_rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
                .unwrap()
                .prepare_typed("Items", &columns, &original_rows, &mut budget(), &|| false)
                .unwrap();
            assert!(!path.exists());
            let published = prepared.publish(&mut budget(), &|| false).unwrap();
            assert!(published.namespace_synced());
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(
                        vec![Value::Int32(1000), Value::Int32(17)],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(
                    "INSERT INTO Items ([Id], [Score]) VALUES (1000, 17)",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (mut session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), after);
            assert!(!bundle.exists());
            saves += 1;
            for null in [false, true] {
                let failure = if typed {
                    let id = if null {
                        Value::Null
                    } else {
                        Value::Int32(1000)
                    };
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(vec![id, Value::Int32(18)], &mut budget(), &|| false)
                        .err()
                        .unwrap()
                } else {
                    let sql = if null {
                        "INSERT INTO Items ([Id], [Score]) VALUES (NULL, 18)"
                    } else {
                        "INSERT INTO Items ([Id], [Score]) VALUES (1000, 18)"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .err()
                        .unwrap()
                };
                index_failure(
                    failure.cause(),
                    if null {
                        IndexConstraintViolation::NullKey
                    } else {
                        IndexConstraintViolation::DuplicateKey
                    },
                );
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(failure.pair_state(), None);
                assert_eq!(fs::read(&path).unwrap(), after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session = failure.into_session();
                refusals += 1;
            }
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), rows);
            let session = editor.into_session();
            assert_eq!(session.last_completed(), Some(receipt));
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
    assert_eq!((saves, refusals), (8, 16));
}
