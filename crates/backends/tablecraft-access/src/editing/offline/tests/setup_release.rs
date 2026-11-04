use super::budget;
use crate::editing::offline::{
    Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate,
};
use std::{fs, io::ErrorKind, path::Path};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable};
use tablecraft_io::{IoError, SharedFile, journal::filesystem::PairError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn stage(session: OfflineWriteSession, before: i32, after: i32) -> StagedUpdate {
    let editor = session
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let snapshot = editor.snapshot();
    assert_eq!(snapshot.rows()[0][1], Value::Int32(before));
    let handle = snapshot.row_handle(0).unwrap();
    editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(after))],
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn collision(staged: StagedUpdate, path: &Path) -> Failure {
    match staged.apply(path, &mut budget(), &|| false) {
        Ok(_) => panic!("expected retained collision"),
        Err(failure) => failure,
    }
}

#[test]
fn release_after_setup_collision_preserves_last_receipt_namespace_and_current_image() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target.mdb");
    fs::write(&path, mutation_fixture::database()).unwrap();
    let session =
        OfflineWriteSession::open(&path, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    let (session, receipt) = stage(session, 10, 12)
        .apply(&root.path().join("completed"), &mut budget(), &|| false)
        .unwrap();
    let current = fs::read(&path).unwrap();
    assert_eq!(session.last_completed(), Some(receipt));

    let bundle = root.path().join("occupied");
    fs::create_dir(&bundle).unwrap();
    fs::write(bundle.join("sentinel"), b"foreign namespace").unwrap();
    let failure = collision(stage(session, 12, 14), &bundle);
    assert_eq!(failure.state(), SessionState::IncompleteSetup);
    assert_eq!(failure.phase(), Phase::Setup);
    assert_eq!(failure.last_completed(), Some(receipt));
    assert_eq!(failure.current_outcome(), None);
    assert!(matches!(
        failure.setup_failure().unwrap().cause(),
        PairError::Io(IoError::Io(ErrorKind::AlreadyExists))
    ));

    failure.into_session().release().unwrap();
    SharedFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), current);
    assert_eq!(
        fs::read(bundle.join("sentinel")).unwrap(),
        b"foreign namespace"
    );
}

#[test]
fn setup_release_error_retains_typed_setup_and_excludes_another_release_or_recovery() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target.mdb");
    let original = mutation_fixture::database();
    fs::write(&path, &original).unwrap();
    let session =
        OfflineWriteSession::open(&path, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    let bundle = root.path().join("occupied");
    fs::write(&bundle, b"foreign journal entry").unwrap();
    let failure = collision(stage(session, 10, 12), &bundle);
    let failure = failure
        .into_session()
        .release_with_setup(|setup| {
            assert!(matches!(
                setup.cause(),
                PairError::Io(IoError::Io(ErrorKind::AlreadyExists))
            ));
            Err(PairError::Io(IoError::Io(ErrorKind::PermissionDenied)))
        })
        .unwrap_err();
    assert_eq!(failure.phase(), Phase::Release);
    assert_eq!(failure.state(), SessionState::ReleaseUncertain);
    assert!(matches!(
        failure.cause(),
        Cause::ReleaseSetup(PairError::Io(IoError::Io(ErrorKind::PermissionDenied)))
    ));
    assert!(matches!(
        failure.setup_failure().unwrap().cause(),
        PairError::Io(IoError::Io(ErrorKind::AlreadyExists))
    ));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.last_completed(), None);

    let failure = failure
        .into_session()
        .release_with_setup(|_| panic!("second unlock must not execute"))
        .unwrap_err();
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert_eq!(failure.state(), SessionState::ReleaseUncertain);
    let failure = match failure
        .into_session()
        .recover_pending(&mut budget(), &|| false)
    {
        Ok(_) => panic!("release uncertainty cannot admit recovery"),
        Err(failure) => failure,
    };
    assert_eq!(failure.state(), SessionState::ReleaseUncertain);
    assert!(failure.setup_failure().is_some());
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read(bundle).unwrap(), b"foreign journal entry");
}
