#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    journal,
    offline::{Cause, OfflineWriteSession, SessionState},
    sql::RecoverableInsertError,
};
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, RecoverableEditError};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, SnapshotError};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::InsertError;
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_text_azl::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(235);

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
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn typed_sql_explicit_property_inserts_and_same_lease_empty_deletes_preserve_literal_images() {
    let mut inserts = 0;
    let mut noops = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.after.is_some() && case.recipe != "empty-insert")
    {
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
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after_bytes());
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            inserts += 1;
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), case.final_rows());
            assert_eq!(
                EditableTable::snapshot(&editor).resolve(old),
                Err(SnapshotError::StaleRow)
            );
            let empty = if typed {
                DeletableTable::plan_deletes(editor, &[], &mut budget(), &|| false).unwrap()
            } else {
                let query = parse_query(
                    &format!("DELETE FROM [{}] WHERE [ID]=-1", case.table),
                    ParseLimits::default(),
                )
                .unwrap();
                editor
                    .into_session()
                    .plan_delete(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((empty.matched_rows(), empty.deleted_rows()), (0, 0));
            let (session, receipt) = empty.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (Some(0), Some(0))
            );
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after_bytes());
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            noops += 1;
            drop(session);
            assert!(ExclusiveFile::open(&path).is_ok());
        }
    }
    assert_eq!((inserts, noops), (24, 24));
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn false_azl_reaches_typed_field_empty_text_without_candidate_namespace_or_lease_loss() {
    let mut refusals = 0;
    for case in cases().into_iter().filter(|case| case.after.is_none()) {
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
            let error = match failure.cause() {
                Cause::Editing(error) => error,
                Cause::InsertPlanning(RecoverableInsertError::Plan(InsertError::Backend(
                    error,
                ))) => error,
                other => panic!("actual INSERT backend cause required: {other:?}"),
            };
            assert!(matches!(
                error,
                journal::Error::Plan(RecoverableEditError::Edit(EditError::Inspect(
                    InspectError::Constraint(ConstraintViolation::Field(
                        FieldConstraintViolation::EmptyText
                    ))
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
            refusals += 1;
        }
    }
    assert_eq!(refusals, 8);
}
