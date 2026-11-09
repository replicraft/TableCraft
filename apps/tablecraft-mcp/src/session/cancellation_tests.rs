//! Synchronize at a real accepted read, never fake a backend or native save result.
use super::tests::{opened, save};
use super::*;
use std::sync::mpsc;
use tablecraft_testkit::fixtures::access::{mutation_fixture, query_expression_edits};

pub(super) fn cancel_after_dispatch(
    session: &mut Session,
) -> (Arc<AtomicBool>, thread::JoinHandle<u64>) {
    let (issued, observed) = mpsc::sync_channel(1);
    let (release, proceed) = mpsc::sync_channel(1);
    session.read_dispatched = Some(Box::new(move |id| {
        issued.send(id).unwrap();
        proceed.recv().unwrap();
    }));
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    let setter = thread::spawn(move || {
        let id = observed.recv().unwrap();
        signal.store(true, Ordering::Release);
        release.send(()).unwrap();
        id
    });
    (cancelled, setter)
}

#[test]
fn accepted_read_cancellation_revokes_publication_without_a_native_write_or_rollback_claim() {
    let original = mutation_fixture::database();
    let (_directory, path, mut session) = opened(&original);
    let revision = session.revision.unwrap();
    let (cancelled, setter) = cancel_after_dispatch(&mut session);
    let result = session.execute_cancellable(
        "read_rows",
        json!({"document_revision": revision, "table": "Items", "offset": 0}),
        cancelled,
    );
    assert!(result.is_err());
    let accepted_read = setter.join().unwrap();
    assert!(!session.worker.publication_is_current(accepted_read));
    let status = session.execute("session_status", json!({})).unwrap();
    assert_eq!(status["document_published"], false);
    assert_eq!(status["source_uncertain"], false);
    assert!(status["active_save_id"].is_null());
    assert!(!session.saves.is_busy());
    assert!(!session.creation.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    session
        .execute("open_database", json!({"path": path}))
        .unwrap();
    let result = session
        .execute(
            "read_rows",
            json!({"document_revision": session.revision.unwrap(), "table": "Items", "offset": 0}),
        )
        .unwrap();
    assert!(result["rows"].is_array());
}

#[test]
fn cancellation_before_execution_preserves_the_existing_source_and_starts_no_creation() {
    let original = mutation_fixture::database();
    let (directory, path, mut session) = opened(&original);
    let revision = session.revision.unwrap();
    let destination = directory.path().join("cancelled.accdb");
    let result = session.execute_cancellable(
        "create_database",
        json!({"path": destination}),
        Arc::new(AtomicBool::new(true)),
    );
    assert!(result.is_err());
    assert_eq!(session.revision, Some(revision));
    assert!(!session.creation.is_busy());
    assert!(!destination.exists());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn cancelled_after_ack_read_keeps_the_accepted_save_id_and_exact_witness_for_retry() {
    let before = query_expression_edits::two_projections();
    let (_directory, path, mut session) = opened(&before);
    let query = &session.catalog["queries"][0];
    let object_id = query["object_id"].as_str().unwrap().to_string();
    let name = query["name"].as_str().unwrap().to_string();
    let revision = session.revision.unwrap();
    let draft = session
        .execute(
            "inspect_query_sql",
            json!({"document_revision": revision, "object_id": object_id, "name": name}),
        )
        .unwrap();
    let review = session.execute("review_query_sql", json!({"document_revision": revision, "object_id": object_id, "name": name, "original_sql": draft["sql"], "proposed_sql": "SELECT 1000 AS Answer,7000 AS [Second];"})).unwrap();
    let id = save(&mut session, "apply_query_sql", review);
    let actual_after = std::fs::read(&path).unwrap();
    let (cancelled, setter) = cancel_after_dispatch(&mut session);
    let result = session.execute_cancellable("acknowledge_save", json!({"save_id": id}), cancelled);
    assert!(result.is_err());
    assert!(
        !session
            .worker
            .publication_is_current(setter.join().unwrap())
    );
    assert!(session.active_save.as_ref().unwrap().acknowledged);
    assert!(session.active_save.as_ref().unwrap().witness.is_some());
    assert!(!session.saves.is_busy());
    let status = session.execute("session_status", json!({})).unwrap();
    assert_eq!(status["active_save_id"], id);
    assert_eq!(status["source_uncertain"], false);
    assert_eq!(std::fs::read(&path).unwrap(), actual_after);
    let result = session
        .execute("acknowledge_save", json!({"save_id": id}))
        .unwrap();
    assert_eq!(result["reopened"], true);
    assert!(session.active_save.is_none());
    assert_eq!(std::fs::read(path).unwrap(), actual_after);
}
