#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
    sql::RecoverableInsertError,
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, SnapshotError};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_text_cardinality::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(230);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn typed_and_sql_text_cardinality_publish_literal_images_with_retained_lease_and_fresh_handles() {
    let mut saves = [0; 2];
    for case in cases().into_iter().filter(|case| {
        if case.action == "insert" {
            case.recipe.starts_with("insert-") && case.recipe != "insert-empty"
        } else {
            matches!(
                case.recipe,
                "delete-null" | "delete-empty" | "delete-case-pair" | "delete-all" | "empty-delete"
            )
        }
    }) {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            let snapshot = EditableTable::snapshot(&editor);
            assert_eq!(snapshot.rows(), sources::original_rows());
            let old = snapshot.row_handle(0).unwrap();
            let receipt;
            let session = if case.action == "insert" {
                let session = editor.into_session();
                let staged = if typed {
                    session
                        .insert_table(case.table, &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(case.insert_values().remove(0), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(&case.sql().unwrap(), ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let result = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                receipt = result.1;
                assert_eq!(receipt.matched_rows(), None);
                saves[0] += 1;
                result.0
            } else {
                let staged = if typed {
                    let handles = case
                        .deleted_slots()
                        .into_iter()
                        .map(|ordinal| snapshot.row_handle(ordinal).unwrap())
                        .collect::<Vec<_>>();
                    DeletableTable::plan_deletes(editor, &handles, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(&case.sql().unwrap(), ParseLimits::default()).unwrap();
                    editor
                        .into_session()
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (staged.matched_rows(), staged.deleted_rows()),
                    (case.changed_rows(), case.changed_rows())
                );
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let result = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                receipt = result.1;
                assert_eq!(receipt.matched_rows(), Some(case.changed_rows()));
                saves[1] += 1;
                result.0
            };
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(case.changed_rows()));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), case.final_rows());
            assert_eq!(
                EditableTable::snapshot(&editor).resolve(old),
                Err(SnapshotError::StaleRow)
            );
            let failure = DeletableTable::plan_deletes(editor, &[old], &mut budget(), &|| false)
                .err()
                .unwrap();
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                    SnapshotError::StaleRow
                )))
            ));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            let session = failure.into_session();
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            drop(session);
            assert!(ExclusiveFile::open(&path).is_ok());
        }
    }
    assert_eq!(saves, [24, 40]);
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn empty_text_insert_requires_explicit_azl_and_keeps_the_original_session_and_namespace() {
    let mut denials = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-empty")
    {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let failure = if typed {
                session
                    .insert_table(case.table, &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(case.insert_values().remove(0), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            } else {
                let query = parse_query(&case.sql().unwrap(), ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            };
            let native = match failure.cause() {
                Cause::Editing(error) => error,
                Cause::InsertPlanning(RecoverableInsertError::Plan(InsertError::Backend(
                    error,
                ))) => error,
                other => panic!("expected actual retained INSERT cause: {other:?}"),
            };
            assert!(matches!(
                native,
                journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::ConstraintCoverage
                )))
            ));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            let session = failure.into_session();
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            drop(session);
            assert!(ExclusiveFile::open(&path).is_ok());
            denials += 1;
        }
    }
    assert_eq!(denials, 8);
}
