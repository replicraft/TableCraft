#![cfg(all(
    feature = "ace16-create",
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Fresh declaration publication and the real retained omitted INSERT consumer.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::{creation::CreationDestination, editing::offline::OfflineWriteSession};
use tablecraft_ace::CreateColumn;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_normal_autonumber as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires source14 and matching Increment creator/omission qualification"]
fn freshly_created_incrementing_table_generates_its_first_row_through_typed_and_sql_sessions() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    for typed in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("created.accdb");
        let bundle = directory.path().join("journal");
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32)
                .with_primary_key("PrimaryId")
                .unwrap()
                .with_incrementing_autonumber()
                .unwrap(),
            CreateColumn::new("Score", ScalarType::Int32),
        ];
        let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
            .unwrap()
            .prepare_typed("Items", &columns, &[], &mut budget(), &|| false)
            .unwrap();
        assert!(!path.exists());
        assert!(
            prepared
                .publish(&mut budget(), &|| false)
                .unwrap()
                .namespace_synced()
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            sources.bytes(5, "increment-zero-before")
        );
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(251), &mut budget(), &|| false)
                .unwrap();
        let staged = if typed {
            session
                .insert_table("Items", &mut budget(), &|| false)
                .unwrap()
                .plan_insert_with_omissions(
                    vec![
                        InsertValue::Omitted,
                        InsertValue::Supplied(Value::Int32(17)),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
        } else {
            let parsed = parse_query(
                "INSERT INTO Items ([Score]) VALUES (17)",
                ParseLimits::default(),
            )
            .unwrap();
            session
                .plan_insert(parsed.query(), &mut budget(), &|| false)
                .unwrap()
        };
        assert_eq!(staged.inserted_rows(), 1);
        assert_eq!(
            fs::read(&path).unwrap(),
            sources.bytes(5, "increment-zero-before")
        );
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(
            fs::read(&path).unwrap(),
            sources.bytes(5, "increment-zero-inserted")
        );
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            EditableTable::snapshot(&editor).rows(),
            vec![vec![Value::Int32(1), Value::Int32(17)]]
        );
        let session = editor.into_session();
        assert_eq!(session.last_completed(), Some(receipt));
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
