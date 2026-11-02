use super::{budget, model, session, write_original};
use std::{fs, io::Cursor};
use tablecraft_access::editing::{
    NativeEditError, NativeEditor, journal,
    offline::{Cause, Phase, SessionState},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{IoError, SharedFile, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;

#[test]
fn typed_edits_refresh_handles_while_retaining_one_target_and_completed_receipt() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let editor = session(&database, &mut budget())
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
    let first_handle = editor.snapshot().row_handle(0).unwrap();
    assert_eq!(
        editor.snapshot().resolve(first_handle).unwrap(),
        &[Value::Int32(1), Value::Int32(10)]
    );
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let staged = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(first_handle, Value::Int32(12))],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
    assert_eq!(fs::read(&database).unwrap(), model::original());
    let (retained, completed) = staged
        .apply(&root.path().join("first"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(completed.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    let editor = retained
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let fresh_handle = editor.snapshot().row_handle(0).unwrap();
    assert_eq!(
        editor.snapshot().resolve(fresh_handle).unwrap(),
        &[Value::Int32(1), Value::Int32(12)]
    );
    assert!(matches!(
        editor.snapshot().resolve(first_handle),
        Err(SnapshotError::StaleRow)
    ));
    let failure = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(first_handle, Value::Int32(17))],
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("previous editor handles must not bind a new snapshot");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(
        failure.cause(),
        Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
            SnapshotError::StaleRow
        )))
    ));
    assert_eq!(failure.state(), SessionState::Ready);
    assert_eq!(failure.last_completed(), Some(completed));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.pair_state(), None);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));

    let editor = failure
        .into_session()
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let handle = editor.snapshot().row_handle(0).unwrap();
    let staged = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(17))],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let (retained, receipt) = staged
        .apply(&root.path().join("second"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(retained.last_completed(), Some(receipt));
    assert_eq!(fs::read(&database).unwrap(), model::with_score(17));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    retained.release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}

#[test]
fn abandoning_typed_snapshot_or_plan_and_committing_noop_preserve_original_bytes() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let editor = session(&database, &mut budget())
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let retained = editor.into_session();
    let editor = retained
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let handle = editor.snapshot().row_handle(0).unwrap();
    let staged = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(12))],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let retained = staged.into_session();
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let editor = retained
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let handle = editor.snapshot().row_handle(0).unwrap();
    let staged = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(10))],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 0));
    let (retained, receipt) = staged
        .apply(&root.path().join("noop"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(receipt.changed_rows(), Some(0));
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    retained.release().unwrap();
}

#[test]
fn duplicate_and_foreign_typed_handles_refuse_before_creating_any_bundle() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let foreign_image = NativeImage::capture(
        &mut Cursor::new(model::original()),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let foreign_editor =
        NativeEditor::new(foreign_image, "Items", &mut budget(), &|| false).unwrap();
    let foreign_handle = foreign_editor.snapshot().row_handle(0).unwrap();
    let mut retained = session(&database, &mut budget());
    for foreign in [false, true] {
        let editor = retained
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let local = editor.snapshot().row_handle(0).unwrap();
        let updates = if foreign {
            vec![CellUpdate::new(foreign_handle, Value::Int32(12))]
        } else {
            vec![
                CellUpdate::new(local, Value::Int32(12)),
                CellUpdate::new(local, Value::Int32(17)),
            ]
        };
        let failure = editor
            .plan_updates("Score", &updates, &mut budget(), &|| false)
            .err()
            .expect("nonlocal or repeated handles must not produce a plan");
        assert_eq!(failure.phase(), Phase::Planning);
        if foreign {
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                    SnapshotError::StaleRow
                )))
            ));
        } else {
            assert!(matches!(failure.cause(), Cause::Editing(_)));
        }
        assert_eq!(failure.pair_state(), None);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(fs::read(&database).unwrap(), model::original());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert!(matches!(
            SharedFile::open(&database),
            Err(IoError::LockBusy)
        ));
        retained = failure.into_session();
    }
    retained.release().unwrap();
}

#[test]
fn readable_ace_outside_closed_content_does_not_grant_typed_writes() {
    let root = tempfile::tempdir().unwrap();
    let case = tablecraft_testkit::fixtures::access::secondary_index::source(
        6,
        tablecraft_testkit::fixtures::access::secondary_index::Population::Distinct,
        false,
    );
    let database = root.path().join("ordinary.accdb");
    fs::write(&database, &case.before).unwrap();
    let editor = session(&database, &mut budget())
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(editor.update_profile(), UpdateProfile::Unavailable);
    assert_eq!(editor.snapshot().rows().len(), case.rows.len());
    for (actual, original) in editor.snapshot().rows().iter().zip(&case.rows) {
        assert_eq!(
            actual,
            &[
                Value::Int32(original.id),
                Value::Int32(original.score.unwrap()),
                Value::Int32(original.guard),
            ]
        );
    }
    let handle = editor.snapshot().row_handle(0).unwrap();
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let failure = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(12))],
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("readable indexed ACE rows do not qualify the closed ordinary family");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(
        failure.cause(),
        Cause::Editing(journal::Error::Native(NativeEditError::Unsupported))
    ));
    assert_eq!(failure.state(), SessionState::Ready);
    assert_eq!(failure.pair_state(), None);
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.last_completed(), None);
    assert_eq!(fs::read(&database).unwrap(), case.before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    failure.into_session().release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}
