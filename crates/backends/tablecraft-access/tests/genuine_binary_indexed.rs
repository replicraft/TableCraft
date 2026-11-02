#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Failure, OfflineWriteSession, SessionState},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_binary_indexed::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(205);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

fn unchanged(path: &Path, bundle: &Path, expected: &[u8]) {
    assert_eq!(fs::read(path).unwrap(), expected);
    assert!(!bundle.exists());
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn editing_error(failure: &Failure) -> &journal::Error {
    match failure.cause() {
        Cause::Editing(error) => error,
        other => panic!("expected actual retained typed editor cause: {other:?}"),
    }
}

fn singles() -> Vec<Case> {
    sources::load_all(verify)
        .into_iter()
        .filter(|case| !case.recipe.starts_with("batch"))
        .collect()
}

#[test]
#[ignore = "requires18 independently native/SDK-qualified Binary source views"]
fn typed_binary_null_empty_segments_and_honest_sql_null_publish_literal_images() {
    let mut saves = [0; 2];
    for case in singles() {
        for typed in [true, false] {
            if !typed && case.recipe != "insert-null" {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let bundle = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();

            let staged = if typed {
                session
                    .insert_table("BinaryKeys", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(
                        sources::insert_values(case.recipe).remove(0),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(
                    "INSERT INTO BinaryKeys ([Marker],[Id],[KeyValue]) VALUES (238,14,NULL)",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            unchanged(&path, &bundle, &case.before);

            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            unchanged(&path, &bundle, &case.after);

            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), case.rows());
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [12, 2]);
}

#[test]
#[ignore = "requires18 independently native/SDK-qualified Binary source views"]
fn binary_insert_refresh_rejects_stale_handles_and_admits_empty_delete() {
    let mut saves = 0;
    for case in singles()
        .into_iter()
        .filter(|case| case.recipe == "insert-empty")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            EditableTable::snapshot(&editor).rows(),
            sources::original_rows()
        );
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
        let stale = EditableTable::snapshot(&editor).row_handle(0).unwrap();
        let session = editor.into_session();

        let staged = session
            .insert_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap()
            .plan_insert(
                sources::insert_values(case.recipe).remove(0),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        unchanged(&path, &bundle, &case.after);
        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let failure = DeletableTable::plan_deletes(editor, &[stale], &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(matches!(
            editing_error(&failure),
            journal::Error::Native(NativeEditError::Snapshot(SnapshotError::StaleRow))
        ));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        unchanged(&path, &bundle, &case.after);

        let editor = failure
            .into_session()
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        let fresh = snapshot.row_handle(0).unwrap();
        assert_eq!(snapshot.resolve(fresh).unwrap(), snapshot.rows()[0]);
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);

        let staged = DeletableTable::plan_deletes(editor, &[], &mut budget(), &|| false).unwrap();
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (0, 0));
        unchanged(&path, &bundle, &case.after);

        let (session, empty_receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(empty_receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(empty_receipt.matched_rows(), Some(0));
        assert_eq!(empty_receipt.changed_rows(), Some(0));
        assert_eq!(session.last_completed(), Some(empty_receipt));
        unchanged(&path, &bundle, &case.after);
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}
