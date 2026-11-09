use crate::session::{
    Session,
    tests::{acknowledge, opened, terminal},
};
use serde_json::json;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::{fixtures::access::mutation_fixture, support::access::restart};

#[test]
fn reserved_pending_and_committed_journals_recover_actual_images_and_preserve_actor_identity() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reserved.mdb");
        let journal = journal_path(&path);
        let images = restart::stage(&path, &journal, committed);
        let mut session = Session::new().unwrap();
        let started = session
            .execute("start_reserved_recovery", json!({"path": path}))
            .unwrap();
        let id = started["save_id"].as_u64().unwrap();
        assert!(
            session
                .execute("start_reserved_recovery", json!({"path": path}))
                .is_err()
        );
        assert!(
            session
                .execute("save_status", json!({"save_id": id + 1000}))
                .is_err()
        );
        let report = terminal(&mut session, id);
        assert_eq!(report["operation"], "Recover");
        assert_eq!(report["outcome"]["kind"], "saved");
        assert_eq!(
            report["outcome"]["receipt"]["known"],
            if committed { "Committed" } else { "RolledBack" }
        );
        assert_eq!(
            report,
            session
                .execute("save_status", json!({"save_id": id}))
                .unwrap()
        );
        assert!(
            session
                .execute("release_save", json!({"save_id": id}))
                .is_err()
        );
        let result = session
            .execute("acknowledge_save", json!({"save_id": id}))
            .unwrap();
        assert_eq!(result["replacement"], committed);
        assert_eq!(result["reopened"], true);
        assert!(session.active_save.is_none());
        assert!(!session.saves.is_busy());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            if committed {
                images.after
            } else {
                images.before
            }
        );
        assert!(!journal.exists());
        let rows = session.execute("read_rows", json!({"document_revision": session.revision.unwrap(), "table": "Items", "offset": 0})).unwrap();
        assert_eq!(
            rows["rows"][0][1]["text"],
            if committed { "17" } else { "10" }
        );
    }
}

#[test]
fn malformed_reserved_artifact_retains_real_release_permissions_and_rejects_fabricated_ids() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("retained.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let journal = journal_path(&path);
    std::fs::write(&journal, b"malformed reserved entry").unwrap();
    let mut session = Session::new().unwrap();
    let started = session
        .execute("start_reserved_recovery", json!({"path": path}))
        .unwrap();
    let id = started["save_id"].as_u64().unwrap();
    let report = terminal(&mut session, id);
    assert_eq!(report["outcome"]["kind"], "failed");
    assert_eq!(report["outcome"]["can_release"], true);
    for operation in [
        "cancel_save",
        "recover_save",
        "release_save",
        "acknowledge_save",
    ] {
        assert!(
            session
                .execute(operation, json!({"save_id": id + 1000}))
                .is_err()
        );
    }
    assert_eq!(
        session
            .execute("save_status", json!({"save_id": id}))
            .unwrap(),
        report
    );
    assert!(
        session
            .execute("acknowledge_save", json!({"save_id": id}))
            .is_err()
    );
    assert!(session.saves.is_busy());
    let released = session
        .execute("release_save", json!({"save_id": id}))
        .unwrap();
    let next = released["save_id"].as_u64().unwrap();
    assert_ne!(next, id);
    assert!(
        session
            .execute("save_status", json!({"save_id": id}))
            .is_err()
    );
    let report = terminal(&mut session, next);
    assert_eq!(report["outcome"]["kind"], "released");
    let acknowledged = session.execute("acknowledge_save", json!({"save_id": next}));
    assert!(
        !session.saves.is_busy(),
        "actual worker ACK releases the retained owner"
    );
    match acknowledged {
        Ok(facts) => {
            assert_eq!(facts["source_verified"], false);
            assert_eq!(facts["source_uncertain"], true);
        }
        Err(_) => {
            assert!(session.active_save.as_ref().unwrap().acknowledged);
        }
    }
    assert!(
        journal.exists(),
        "release is not permission to delete an unqualified artifact"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn explicit_save_cancellation_keeps_the_real_report_and_never_infers_rollback_from_the_request() {
    let original = mutation_fixture::database();
    let (_directory, path, mut session) = opened(&original);
    let revision = session.revision.unwrap();
    let draft = session.execute("begin_edit", json!({"document_revision": revision, "table": "Items", "column": "Score", "parent_ordinal": 0})).unwrap();
    let review = session
        .execute(
            "review_edit",
            json!({"document_revision": revision, "draft_id": draft["draft_id"], "text": "17"}),
        )
        .unwrap();
    let started = session
        .execute("apply_edit", json!({"review_id": review["review_id"]}))
        .unwrap();
    let mut id = started["save_id"].as_u64().unwrap();
    let cancellation = session
        .execute("cancel_save", json!({"save_id": id}))
        .unwrap();
    assert!(cancellation["cancellation_requested"].is_boolean());
    assert_eq!(
        session.execute("session_status", json!({})).unwrap()["active_save_id"],
        id
    );
    let mut report = terminal(&mut session, id);
    if report["outcome"]["kind"] == "failed" {
        let operation = if report["outcome"]["can_recover"] == true {
            Some("recover_save")
        } else if report["outcome"]["can_release"] == true {
            Some("release_save")
        } else {
            None
        };
        if let Some(operation) = operation {
            let continued = session.execute(operation, json!({"save_id": id})).unwrap();
            id = continued["save_id"].as_u64().unwrap();
            report = terminal(&mut session, id);
        }
    }
    assert_ne!(report["status"], "pending");
    assert_eq!(report["save_id"], id);
    if report["outcome"]["kind"] == "saved" && report["outcome"]["receipt"]["known"] == "Committed"
    {
        acknowledge(&mut session, id, &path);
        let rows = session.execute("read_rows", json!({"document_revision": session.revision.unwrap(), "table": "Items", "offset": 0})).unwrap();
        assert_eq!(rows["rows"][0][1]["text"], "17");
    } else {
        // Consume only the real terminal envelope; it may certify original or
        // keep unknown/reopen restrictions. A cancellation Boolean certifies neither.
        let _ = session.execute("acknowledge_save", json!({"save_id": id}));
    }
}
