#![cfg(all(
    feature = "ace16-create",
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Real fresh creation and retained typed/SQL omission-aware sessions.
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::{
    creation::{CreationDestination, CreationError},
    editing::{
        journal,
        offline::{Cause, OfflineWriteSession, Phase},
    },
};
use tablecraft_ace::{
    CreateColumn, CreateDatabaseError, EditError, RecoverableEditError, WriteBlocker,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::creation_date_defaults::{self as fixtures, Case};

const SCOPE: CatalogScope = CatalogScope::new(231);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn create(path: &Path, case: Case, sources: &fixtures::Sources) {
    let literal = fixtures::date(case.bits);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::new("When", ScalarType::DateTime)
            .with_default_literal(&literal)
            .unwrap(),
    ];
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
    assert_eq!(fs::read(path).unwrap(), sources.bytes(5, case, "before"));
}

#[test]
#[ignore = "requires source36 native parser/bit/Source qualification before matching producers"]
fn newly_created_dates_flow_through_twenty_four_retained_typed_and_sql_saves() {
    let sources = sources();
    let mut saves = 0;
    for case in fixtures::CASES {
        for phase in ["omitted", "supplied-null"] {
            for typed in [true, false] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("created.accdb");
                let bundle = directory.path().join("journal");
                create(&path, case, &sources);
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let fields = if phase == "omitted" {
                        vec![InsertValue::Supplied(Value::Int32(2)), InsertValue::Omitted]
                    } else {
                        vec![
                            InsertValue::Supplied(Value::Int32(3)),
                            InsertValue::Supplied(Value::Null),
                        ]
                    };
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert_with_omissions(fields, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let text = if phase == "omitted" {
                        "INSERT INTO Items ([Id]) VALUES (2)"
                    } else {
                        "INSERT INTO Items ([Id], [When]) VALUES (3, NULL)"
                    };
                    let parsed = parse_query(text, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(parsed.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), sources.bytes(5, case, "before"));
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                let expected = sources.bytes(5, case, phase);
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    EditableTable::snapshot(&editor).rows(),
                    fixtures::expected_rows(case, phase)
                );
                let session = editor.into_session();
                assert_eq!(session.last_completed(), Some(receipt));
                // A later planning refusal keeps the same completion receipt,
                // exact completed image, empty bundle namespace and held lease.
                let failure = session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert_with_omissions(
                        vec![InsertValue::Supplied(Value::Int32(99))],
                        &mut budget(),
                        &|| false,
                    )
                    .err()
                    .unwrap();
                assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                        EditError::Unsupported(WriteBlocker::Column)
                    )))
                ));
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 24);
}

#[test]
fn nonreversible_dates_never_publish_a_destination_or_touch_siblings() {
    for bits in fixtures::ALIAS_BITS {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unpublished.accdb");
        let sibling = directory.path().join("sentinel");
        fs::write(&sibling, b"unchanged").unwrap();
        let literal = fixtures::date(bits);
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32),
            CreateColumn::new("When", ScalarType::DateTime)
                .with_default_literal(&literal)
                .unwrap(),
        ];
        let row = fixtures::original_row();
        let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            destination.prepare_typed("Items", &columns, &[row.as_slice()], &mut budget(), &|| {
                false
            }),
            Err(CreationError::Create(CreateDatabaseError::Declaration))
        ));
        assert!(!path.exists());
        assert_eq!(fs::read(&sibling).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
