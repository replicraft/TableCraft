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
    },
};
use tablecraft_ace::{
    CreateColumn, EditError, IndexConstraintViolation, InspectError, RecoverableEditError,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_creation_integral_primary::{
    self as sources, Family,
};

const SCOPE: CatalogScope = CatalogScope::new(223);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn index_failure(cause: &Cause, expected: IndexConstraintViolation) {
    assert!(matches!(
        cause,
        Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
            EditError::Inspect(InspectError::IndexConstraint(actual))
        ))) if *actual == expected
    ));
}

#[test]
fn newly_created_wide_manual_primary_keeps_exact_typed_keys_and_literal_insert_delete_images() {
    let mut publications = 0;
    let mut saves = 0;
    let mut refusals = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        let Some((inserted, inserted_rows)) = sources::inserted(&source) else {
            continue;
        };
        let (deleted, remaining) = sources::deleted_after_insert(&source).unwrap();
        let scalar = match source.family {
            Family::Int64 => ScalarType::Int64,
            Family::Currency => ScalarType::Currency,
        };
        let columns = [
            CreateColumn::new("Id", scalar)
                .with_primary_key("PrimaryId")
                .unwrap(),
            CreateColumn::new("Score", ScalarType::Int32),
        ];
        let original_rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("created.accdb");
        let bundle = directory.path().join("journal");
        let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
            .unwrap()
            .prepare_typed("Items", &columns, &original_rows, &mut budget(), &|| false)
            .unwrap();
        assert!(!path.exists());
        assert!(
            prepared
                .publish(&mut budget(), &|| false)
                .unwrap()
                .namespace_synced()
        );
        assert_eq!(fs::read(&path).unwrap(), source.bytes);
        publications += 1;

        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let staged = session
            .insert_table("Items", &mut budget(), &|| false)
            .unwrap()
            .plan_insert(
                vec![source.family.value(10001), Value::Int32(17)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(staged.inserted_rows(), 1);
        assert_eq!(fs::read(&path).unwrap(), source.bytes);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (mut session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), inserted);
        assert!(!bundle.exists());
        saves += 1;

        for (key, expected) in [
            (
                source.family.value(10001),
                IndexConstraintViolation::DuplicateKey,
            ),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let failure = session
                .insert_table("Items", &mut budget(), &|| false)
                .unwrap()
                .plan_insert(vec![key, Value::Int32(18)], &mut budget(), &|| false)
                .err()
                .unwrap();
            index_failure(failure.cause(), expected);
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), inserted);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            session = failure.into_session();
            refusals += 1;
        }

        // Resolve a fresh snapshot handle, never reuse a pre-INSERT ordinal token.
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(snapshot.rows(), inserted_rows);
        let row = snapshot.row_handle(source.rows.len()).unwrap();
        let staged =
            DeletableTable::plan_deletes(editor, &[row], &mut budget(), &|| false).unwrap();
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
        assert_eq!(fs::read(&path).unwrap(), inserted);
        let (session, removed) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(removed.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(removed.matched_rows(), Some(1));
        assert_eq!(removed.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(removed));
        assert_eq!(fs::read(&path).unwrap(), deleted);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(EditableTable::snapshot(&editor).rows(), remaining);
        editor.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!((publications, saves, refusals), (4, 8, 8));
}
