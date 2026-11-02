#![cfg(all(
    feature = "ace16-create",
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::fs;
use tablecraft_access::{creation::CreationDestination, editing::offline::OfflineWriteSession};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_native17_sessions;

const SCOPE: CatalogScope = CatalogScope::new(204);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn freshly_published_database_uses_normal_typed_and_sql_insert_delete_without_creator_permission() {
    // Construct source expectations first, from independent original/request laws.
    let expected = canonical_native17_sessions::source();
    for typed in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("created.accdb");
        let bundle = directory.path().join("journal");
        let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
        let prepared = destination
            .prepare(
                "Items",
                &["Id", "Score"],
                &[&[1, 10]],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
        let receipt = prepared.publish(&mut budget(), &|| false).unwrap();
        assert!(receipt.namespace_synced());
        assert_eq!(fs::read(&path).unwrap(), expected.before);

        // The normal owner receives only a path and fresh bytes. No creator
        // object, provenance, program selection or stored permission crosses.
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let staged = if typed {
            session
                .insert_table("Items", &mut budget(), &|| false)
                .unwrap()
                .plan_insert(
                    vec![Value::Int32(2), Value::Int32(20)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
        } else {
            let query = parse_query(
                "INSERT INTO Items ([Id], [Score]) VALUES (2, 20)",
                ParseLimits::default(),
            )
            .unwrap();
            session
                .plan_insert(query.query(), &mut budget(), &|| false)
                .unwrap()
        };
        assert_eq!(staged.inserted_rows(), 1);
        assert_eq!(fs::read(&path).unwrap(), expected.before);
        assert!(!bundle.exists());
        let (session, inserted) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(inserted.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(inserted.matched_rows(), None);
        assert_eq!(inserted.changed_rows(), Some(1));
        assert_eq!(fs::read(&path).unwrap(), expected.inserted);
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(
            snapshot.rows(),
            [
                vec![Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(2), Value::Int32(20)]
            ]
        );
        let staged = if typed {
            let handle = snapshot.row_handle(1).unwrap();
            DeletableTable::plan_deletes(editor, &[handle], &mut budget(), &|| false).unwrap()
        } else {
            let session = editor.into_session();
            let query =
                parse_query("DELETE FROM Items WHERE [Id]=2", ParseLimits::default()).unwrap();
            session
                .plan_delete(query.query(), &mut budget(), None, &|| false)
                .unwrap()
        };
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
        assert_eq!(fs::read(&path).unwrap(), expected.inserted);
        let (session, deleted) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(deleted.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(deleted.matched_rows(), Some(1));
        assert_eq!(deleted.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(deleted));
        assert_eq!(fs::read(&path).unwrap(), expected.deleted);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            EditableTable::snapshot(&editor).rows(),
            [vec![Value::Int32(1), Value::Int32(10)]]
        );
        editor.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
