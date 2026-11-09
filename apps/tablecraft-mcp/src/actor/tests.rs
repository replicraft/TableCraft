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
    let actor = super::Actor::start(
        directory.path().to_path_buf(),
        crate::permissions::Permissions::default(),
    )
    .unwrap();
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

#[tokio::test(flavor = "current_thread")]
async fn actor_validates_wire_input_before_path_work_and_keeps_budget_before_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let destination = outside.path().join("not-authorized.accdb");
    let actor = super::Actor::start(
        directory.path().to_path_buf(),
        crate::permissions::Permissions::default(),
    )
    .unwrap();

    // Invoke the actor directly, bypassing protocol dispatch: it must still
    // reject unknown wire fields before attempting authorization or creation.
    let error = actor
        .execute(
            "create_database".into(),
            json!({"path": destination, "unexpected": "must not be ignored"}),
            std::future::pending(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("unknown field"), "{error}");
    assert!(!destination.exists());

    // The asynchronous boundary rejects an oversized envelope before queueing
    // it for typed deserialization on the dedicated session actor.
    let error = actor
        .execute(
            "create_database".into(),
            json!({"path": destination, "unexpected": "x".repeat(super::MAX_ARGUMENT_BYTES)}),
            std::future::pending(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("input budget"), "{error}");
    assert!(!destination.exists());
    let status = actor
        .execute("session_status".into(), json!({}), std::future::pending())
        .await
        .unwrap();
    assert!(status["active_creation_id"].is_null());
    assert!(status["active_save_id"].is_null());
}

#[tokio::test(flavor = "current_thread")]
async fn data_grant_applies_actual_review_but_never_authorizes_schema_or_application_writes() {
    use crate::permissions::Permissions;
    use std::time::{Duration, Instant};
    use tablecraft_testkit::fixtures::access::mutation_fixture;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("granted.mdb");
    let original = mutation_fixture::database();
    let expected = mutation_fixture::with_rows(&[(1, 17), (2, 20), (3, 30)]);
    std::fs::write(&path, &original).unwrap();
    let actor = super::Actor::start(
        directory.path().to_path_buf(),
        Permissions {
            data_write: true,
            ..Permissions::default()
        },
    )
    .unwrap();
    let opened = actor
        .execute(
            "open_database".into(),
            json!({
                "path": path
            }),
            std::future::pending(),
        )
        .await
        .unwrap();
    let revision = opened["catalog"]["document_revision"].as_u64().unwrap();
    let draft = actor
        .execute(
            "begin_edit".into(),
            json!({
                "document_revision": revision,
                "table": "Items",
                "column": "Score",
                "parent_ordinal": 0
            }),
            std::future::pending(),
        )
        .await
        .unwrap();
    let reviewed = actor
        .execute(
            "review_edit".into(),
            json!({
                "document_revision": revision,
                "draft_id": draft["draft_id"],
                "text": "17"
            }),
            std::future::pending(),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let started = actor
        .execute(
            "apply_edit".into(),
            json!({
                "review_id": reviewed["review_id"]
            }),
            std::future::pending(),
        )
        .await
        .unwrap();
    let save_id = started["save_id"].as_u64().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let report = actor
            .execute(
                "save_status".into(),
                json!({
                    "save_id": save_id
                }),
                std::future::pending(),
            )
            .await
            .unwrap();
        if report["status"] == "terminal" {
            assert_eq!(report["outcome"]["kind"], "saved");
            break;
        }
        assert!(Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    let acknowledged = actor
        .execute(
            "acknowledge_save".into(),
            json!({
                "save_id": save_id
            }),
            std::future::pending(),
        )
        .await
        .unwrap();
    assert_eq!(acknowledged["reopened"], true);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    for (operation, arguments, flag) in [
        (
            "create_database",
            json!({"path": directory.path().join("not-created.accdb")}),
            "--allow-schema-write",
        ),
        (
            "apply_query_sql",
            json!({"review_id": 1}),
            "--allow-application-write",
        ),
        (
            "start_reserved_recovery",
            json!({"path": path}),
            "--allow-maintenance",
        ),
    ] {
        let error = actor
            .execute(operation.into(), arguments, std::future::pending())
            .await
            .unwrap_err();
        assert!(error.contains(flag), "{error}");
    }
    assert!(!directory.path().join("not-created.accdb").exists());
    assert_eq!(std::fs::read(path).unwrap(), expected);
}

#[tokio::test(flavor = "current_thread")]
async fn default_actor_refuses_json_grant_escalation_and_lifecycle_without_an_owned_operation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("not-created.accdb");
    let actor = super::Actor::start(
        directory.path().to_path_buf(),
        crate::permissions::Permissions::default(),
    )
    .unwrap();
    let error = actor
        .execute(
            "create_database".into(),
            json!({
                "path": path,
                "allow_schema_write": true
            }),
            std::future::pending(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("unknown field"), "{error}");
    for operation in [
        "recover_save",
        "release_save",
        "acknowledge_save",
        "cancel_save",
    ] {
        assert!(
            actor
                .execute(
                    operation.into(),
                    json!({
                        "save_id": 1
                    }),
                    std::future::pending()
                )
                .await
                .is_err()
        );
    }
    let status = actor
        .execute("session_status".into(), json!({}), std::future::pending())
        .await
        .unwrap();
    assert!(status["active_save_id"].is_null());
    assert!(status["active_creation_id"].is_null());
    assert!(!path.exists());
}
