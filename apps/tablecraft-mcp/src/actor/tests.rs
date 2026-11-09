use super::authorize_paths;
use serde_json::json;
use std::fs;

#[test]
fn authorizes_existing_sources_and_fresh_bundles_within_the_explicit_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("example.accdb");
    fs::write(&database, b"not opened by path authorization").unwrap();

    assert!(authorize_paths(&root, &mut json!({"path": database})).is_ok());
    assert!(authorize_paths(&root, &mut json!({"bundle": root.join("new-bundle")})).is_ok());
}

#[test]
fn refuses_relative_and_outside_paths_for_both_read_and_save_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let external_file = outside.path().join("external.accdb");
    fs::write(&external_file, b"external").unwrap();

    assert!(authorize_paths(&root, &mut json!({"path": "relative.accdb"})).is_err());
    assert!(authorize_paths(&root, &mut json!({"path": external_file})).is_err());
    assert!(
        authorize_paths(
            &root,
            &mut json!({"bundle": outside.path().join("new-bundle")})
        )
        .is_err()
    );
    assert!(!outside.path().join("new-bundle").exists());
}

#[cfg(unix)]
#[test]
fn resolves_symlinks_before_authorizing_existing_and_fresh_paths() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    fs::write(outside.path().join("external.accdb"), b"external").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("linked-directory")).unwrap();

    assert!(
        authorize_paths(
            &root,
            &mut json!({"path": root.join("linked-directory/external.accdb")})
        )
        .is_err()
    );
    assert!(
        authorize_paths(
            &root,
            &mut json!({"bundle": root.join("linked-directory/new-bundle")})
        )
        .is_err()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_ready_before_queue_acceptance_never_creates_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("cancelled.accdb");
    let actor = super::Actor::start(directory.path().to_path_buf()).unwrap();
    let result = actor
        .execute(
            "create_database".into(),
            json!({"path": destination}),
            std::future::ready(()),
        )
        .await;
    assert!(result.is_err());
    assert!(!destination.exists());
    let status = actor
        .execute("session_status".into(), json!({}), std::future::pending())
        .await
        .unwrap();
    assert!(status["active_creation_id"].is_null());
    assert!(status["active_save_id"].is_null());
}

#[test]
fn dropping_request_guard_revokes_only_its_own_read_signal() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let current = Arc::new(AtomicBool::new(false));
    let other = Arc::new(AtomicBool::new(false));
    let guard = super::CancelOnDrop {
        cancelled: Arc::clone(&current),
    };
    drop(guard);
    assert!(current.load(Ordering::Acquire));
    assert!(!other.load(Ordering::Acquire));
}
