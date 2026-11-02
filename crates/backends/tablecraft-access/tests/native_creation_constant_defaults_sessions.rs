#![cfg(all(
    feature = "ace16-create",
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, path::Path};
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
use tablecraft_db::{EditableTable, InsertValue, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_schema::{FieldConstraintViolation, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::creation_constant_defaults as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(227);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn create(path: &Path, case: &str, sources: &fixtures::Sources) {
    let defaults = fixtures::defaults(case);
    let columns = columns(case, &defaults);
    let row = fixtures::original_row();
    let prepared = CreationDestination::capture(path, &mut budget(), &|| false)
        .unwrap()
        .prepare_typed("Items", &columns, &[row.as_slice()], &mut budget(), &|| {
            false
        })
        .unwrap();
    assert!(!path.exists());
    assert!(
        prepared
            .publish(&mut budget(), &|| false)
            .unwrap()
            .namespace_synced()
    );
    assert_eq!(fs::read(path).unwrap(), sources.bytes(case, "before"));
}

fn request(phase: &str) -> (Vec<InsertValue>, &'static str) {
    match phase {
        "omitted-constants" => (
            vec![
                InsertValue::Supplied(Value::Int32(2)),
                InsertValue::Omitted,
                InsertValue::Omitted,
                InsertValue::Omitted,
                InsertValue::Omitted,
            ],
            "INSERT INTO Items ([Id]) VALUES (2)",
        ),
        "supplied-null" => (
            vec![
                InsertValue::Supplied(Value::Int32(3)),
                InsertValue::Omitted,
                InsertValue::Supplied(Value::Null),
                InsertValue::Supplied(Value::Null),
                InsertValue::Supplied(Value::Null),
            ],
            "INSERT INTO Items ([Id], [Large], [Money], [Caption]) VALUES (3, NULL, NULL, NULL)",
        ),
        "supplied-overrides" => (
            vec![
                InsertValue::Supplied(Value::Int32(4)),
                InsertValue::Supplied(Value::Boolean(false)),
                InsertValue::Omitted,
                InsertValue::Omitted,
                InsertValue::Supplied(Value::Text("override".into())),
            ],
            "INSERT INTO Items ([Id], [Flag], [Caption]) VALUES (4, False, 'override')",
        ),
        _ => panic!("unknown original presence request"),
    }
}

#[test]
#[ignore = "requires all 28 frozen source views, qualified before matching creators"]
fn actual_created_constants_flow_through_retained_typed_and_sql_presence_consumers() {
    let sources = sources();
    let mut saves = 0;
    for case in fixtures::CASES {
        for phase in ["omitted-constants", "supplied-null", "supplied-overrides"] {
            if case == "required-empty" && phase != "supplied-overrides" {
                continue;
            }
            for typed in [true, false] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("created.accdb");
                let bundle = directory.path().join("journal");
                create(&path, case, &sources);
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let (values, sql) = request(phase);
                let staged = if typed {
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert_with_omissions(values, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(parsed.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), sources.bytes(case, "before"));
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), sources.bytes(case, phase));
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                let actual = EditableTable::snapshot(&editor).rows();
                assert_eq!(actual.len(), 2);
                assert_eq!(actual[0], fixtures::original_row());
                let mut expected = vec![match phase {
                    "omitted-constants" => Value::Int32(2),
                    "supplied-null" => Value::Int32(3),
                    _ => Value::Int32(4),
                }];
                let defaults = fixtures::defaults(case);
                match phase {
                    "omitted-constants" => expected.extend(defaults),
                    "supplied-null" => expected.extend([
                        defaults[0].clone(),
                        Value::Null,
                        Value::Null,
                        Value::Null,
                    ]),
                    _ => expected.extend([
                        Value::Boolean(false),
                        defaults[1].clone(),
                        defaults[2].clone(),
                        Value::Text("override".into()),
                    ]),
                }
                assert_eq!(actual[1], expected);
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

#[test]
#[ignore = "requires all 28 frozen source views, qualified before matching creators"]
fn required_and_empty_default_failures_retain_original_bytes_and_the_same_lease() {
    let sources = sources();
    for (phase, violation) in [
        ("omitted-constants", FieldConstraintViolation::EmptyText),
        ("supplied-null", FieldConstraintViolation::Required),
    ] {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("created.accdb");
            let bundle = directory.path().join("journal");
            create(&path, "required-empty", &sources);
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let (values, sql) = request(phase);
            let failure = if typed {
                session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert_with_omissions(values, &mut budget(), &|| false)
                    .err()
                    .unwrap()
            } else {
                let parsed = parse_query(sql, ParseLimits::default()).unwrap();
                session
                    .plan_insert(parsed.query(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            };
            let error = match failure.cause() {
                Cause::Editing(error) => error,
                Cause::InsertPlanning(RecoverableInsertError::Plan(InsertError::Backend(
                    error,
                ))) => error,
                _ => panic!("expected native constraint refusal"),
            };
            assert!(
                matches!(error, journal::Error::Plan(RecoverableEditError::Edit(EditError::Inspect(InspectError::Constraint(ConstraintViolation::Field(actual))))) if *actual == violation)
            );
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(
                fs::read(&path).unwrap(),
                sources.bytes("required-empty", "before")
            );
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
        }
    }
}

#[test]
fn oversized_escaped_default_refuses_before_destination_publication() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unpublished.accdb");
    let literal = Value::Text("\"".repeat(127));
    let columns = [tablecraft_ace::CreateColumn::short_text("Caption", 255)
        .unwrap()
        .with_default_literal(&literal)
        .unwrap()];
    let rows: [&[Value]; 0] = [];
    let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        destination.prepare_typed("Items", &columns, &rows, &mut budget(), &|| false),
        Err(tablecraft_access::creation::CreationError::Create(
            tablecraft_ace::CreateDatabaseError::Declaration
        ))
    ));
    assert!(!path.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

fn columns<'a>(case: &str, defaults: &'a [Value]) -> Vec<CreateColumn<'a>> {
    let mut caption = CreateColumn::short_text("Caption", 16)
        .unwrap()
        .with_allow_zero_length(case != "required-empty")
        .unwrap()
        .with_default_literal(&defaults[3])
        .unwrap();
    if case == "required-empty" {
        caption = caption.with_required(true);
    }
    vec![
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::new("Flag", ScalarType::Boolean)
            .with_default_literal(&defaults[0])
            .unwrap(),
        CreateColumn::new("Large", ScalarType::Int64)
            .with_default_literal(&defaults[1])
            .unwrap(),
        CreateColumn::new("Money", ScalarType::Currency)
            .with_default_literal(&defaults[2])
            .unwrap(),
        caption,
    ]
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}
