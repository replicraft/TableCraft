#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, RecoverableEditError, WriteBlocker,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as sources, Recipe,
};

const SCOPE: CatalogScope = CatalogScope::new(123);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn locked(path: &std::path::Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn adapter_error(failure: &Failure) -> &journal::Error {
    match failure.cause() {
        Cause::Editing(error) => error,
        cause => panic!("expected an actual retained typed editor error: {cause:?}"),
    }
}

#[test]
fn typed_indexed_sessions_publish_sixteen_complete_nullable_tree_alias_and_noop_images() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::PromoteNull,
            Recipe::ClearValue,
            Recipe::UniqueSwap,
            Recipe::Aliases,
            Recipe::ManualTree,
            Recipe::PrefixLoss,
            Recipe::Empty,
            Recipe::NullNoop,
        ] {
            let case = sources::source(header, recipe);
            let expected = case.expected.as_ref().unwrap();
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            assert_eq!(editor.snapshot().rows(), case.rows);
            let updates = case
                .updates
                .iter()
                .map(|request| {
                    assert_eq!(case.rows[request.row][0], value(request.expected));
                    CellUpdate::new(
                        editor.snapshot().row_handle(request.row).unwrap(),
                        value(request.replacement),
                    )
                })
                .collect::<Vec<_>>();
            let staged = editor
                .plan_updates("Id", &updates, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                (staged.matched_rows(), staged.changed_rows()),
                (case.updates.len(), case.changed_rows)
            );
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            locked(&path);
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(&fs::read(&path).unwrap(), expected);
            assert!(!bundle.exists());
            locked(&path);
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.final_rows);
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 16);
}

#[test]
fn typed_refusals_and_real_stale_handles_keep_files_leases_and_last_receipts() {
    for header in [5, 6] {
        for recipe in [
            Recipe::GeneratedTarget,
            Recipe::UniqueConflict,
            Recipe::PrimaryNull,
        ] {
            let case = sources::source(header, recipe);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            let requests = case
                .updates
                .iter()
                .map(|request| {
                    CellUpdate::new(
                        editor.snapshot().row_handle(request.row).unwrap(),
                        value(request.replacement),
                    )
                })
                .collect::<Vec<_>>();
            let failure = editor
                .plan_updates("Id", &requests, &mut budget(), &|| false)
                .err()
                .expect("the typed request cannot return a staged update");
            match recipe {
                Recipe::GeneratedTarget => assert!(matches!(
                    adapter_error(&failure),
                    journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(
                        WriteBlocker::Column
                    )))
                )),
                Recipe::UniqueConflict => assert!(matches!(
                    adapter_error(&failure),
                    journal::Error::Plan(RecoverableEditError::Edit(EditError::Inspect(
                        InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
                    )))
                )),
                Recipe::PrimaryNull => assert!(matches!(
                    adapter_error(&failure),
                    journal::Error::Plan(RecoverableEditError::Edit(EditError::Inspect(
                        InspectError::IndexConstraint(IndexConstraintViolation::NullKey)
                    )))
                )),
                _ => unreachable!(),
            }
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&path);
            failure.into_session().release().unwrap();
        }

        let case = sources::source(header, Recipe::ManualSeven);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Id",
                &[CellUpdate::new(stale, Value::Int32(7))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "Id",
                &[CellUpdate::new(stale, Value::Int32(8))],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("an old snapshot handle cannot bind the refreshed row");
        assert!(matches!(
            adapter_error(&failure),
            journal::Error::Native(NativeEditError::Snapshot(SnapshotError::StaleRow))
        ));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(&fs::read(&path).unwrap(), case.expected.as_ref().unwrap());
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}
