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
    ConstraintViolation, CreateColumn, EditError, InspectError, RecoverableEditError,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_schema::{FieldConstraintViolation, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_creation_defaults as sources;

const SCOPE: CatalogScope = CatalogScope::new(220);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn create(path: &std::path::Path, source: &sources::Source) {
    let literal = source.omitted_score.map_or(Value::Null, Value::Int32);
    let mut score = CreateColumn::new("Score", ScalarType::Int32);
    if source.expression.is_some() {
        score = score.with_default_literal(&literal).unwrap();
    }
    if let Some(required) = source.required {
        score = score.with_required(required);
    }
    let columns = [CreateColumn::new("Id", ScalarType::Int32), score];
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepared = CreationDestination::capture(path, &mut budget(), &|| false)
        .unwrap()
        .prepare_typed("Items", &columns, &rows, &mut budget(), &|| false)
        .unwrap();
    assert!(!path.exists());
    let receipt = prepared.publish(&mut budget(), &|| false).unwrap();
    assert!(receipt.namespace_synced());
    assert_eq!(fs::read(path).unwrap(), source.bytes);
}

#[test]
fn newly_created_defaults_flow_through_real_typed_and_sql_presence_and_complete_consumers() {
    let mut saves = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        if source.omitted_refused {
            continue;
        }
        for supplied_null in [false, true] {
            let (expected, rows) = sources::inserted(&source, supplied_null).unwrap();
            for typed in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("created.accdb");
                let bundle = directory.path().join("journal");
                create(&path, &source);
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let inserter = session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(
                        inserter
                            .columns()
                            .iter()
                            .map(|column| column.name())
                            .collect::<Vec<_>>(),
                        ["Id", "Score"]
                    );
                    if supplied_null {
                        inserter
                            .plan_insert(
                                vec![Value::Int32(25), Value::Null],
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap()
                    } else {
                        inserter
                            .plan_insert_with_omissions(
                                vec![
                                    InsertValue::Supplied(Value::Int32(24)),
                                    InsertValue::Omitted,
                                ],
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap()
                    }
                } else {
                    let sql = if supplied_null {
                        "INSERT INTO Items ([Id], [Score]) VALUES (25, NULL)"
                    } else {
                        "INSERT INTO Items ([Id]) VALUES (24)"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(EditableTable::snapshot(&editor).rows(), rows);
                let session = editor.into_session();
                assert_eq!(session.last_completed(), Some(receipt));
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 20);
}

fn required(cause: &Cause) {
    let error = match cause {
        Cause::Editing(error) => error,
        Cause::InsertPlanning(RecoverableInsertError::Plan(InsertError::Backend(error))) => error,
        _ => panic!("expected the precise native field-constraint boundary"),
    };
    assert!(matches!(
        error,
        journal::Error::Plan(RecoverableEditError::Edit(EditError::Inspect(
            InspectError::Constraint(ConstraintViolation::Field(
                FieldConstraintViolation::Required
            ))
        )))
    ));
}

#[test]
fn required_null_default_does_not_repair_omission_or_supplied_null_and_retains_the_lease() {
    let source = sources::source(sources::Case::RequiredNull);
    let mut refusals = 0;
    for supplied_null in [false, true] {
        for typed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("created.accdb");
            let bundle = directory.path().join("journal");
            create(&path, &source);
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let failure = if typed {
                let inserter = session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap();
                if supplied_null {
                    inserter
                        .plan_insert(vec![Value::Int32(25), Value::Null], &mut budget(), &|| {
                            false
                        })
                        .err()
                        .unwrap()
                } else {
                    inserter
                        .plan_insert_with_omissions(
                            vec![
                                InsertValue::Supplied(Value::Int32(24)),
                                InsertValue::Omitted,
                            ],
                            &mut budget(),
                            &|| false,
                        )
                        .err()
                        .unwrap()
                }
            } else {
                let sql = if supplied_null {
                    "INSERT INTO Items ([Id], [Score]) VALUES (25, NULL)"
                } else {
                    "INSERT INTO Items ([Id]) VALUES (24)"
                };
                let query = parse_query(sql, ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            };
            required(failure.cause());
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 4);
}

#[path = "native_creation_defaults_sessions/lease.rs"]
mod lease;
