use crate::session::{
    Session,
    tests::{acknowledge, opened, save, terminal},
};
use serde_json::json;
use std::path::PathBuf;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

struct FailedInsert {
    directory: tempfile::TempDir,
    path: PathBuf,
    session: Session,
    original: Vec<u8>,
    changed: Vec<u8>,
    revision: u64,
    old_review: u64,
    id: u64,
}

fn failed_insert() -> FailedInsert {
    let original = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let (directory, path, mut session) = opened(&original);
    let revision = session.revision.unwrap();
    let draft = session
        .execute(
            "begin_insert",
            json!({
                "document_revision": revision,
                "table": "Items"
            }),
        )
        .unwrap();
    let review = session
        .execute(
            "review_insert",
            json!({
                "document_revision": revision,
                "draft_id": draft["draft_id"],
                "inputs": [{
                    "text": "4"
                },
                {
                    "text": "50"
                }]
            }),
        )
        .unwrap();
    let old_review = review["review_id"].as_u64().unwrap();
    let mut changed = original.clone();
    changed.push(0);
    std::fs::write(&path, &changed).unwrap();
    let started = session
        .execute(
            "apply_insert",
            json!({
                "review_id": old_review
            }),
        )
        .unwrap();
    let mut id = started["save_id"].as_u64().unwrap();
    let report = terminal(&mut session, id);
    assert_eq!(report["outcome"]["kind"], "failed");
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id
                })
            )
            .is_err(),
        "the real save actor must be acknowledged first"
    );
    if report["outcome"]["can_release"] == true {
        assert!(
            session
                .execute(
                    "acknowledge_save",
                    json!({
                        "save_id": id
                    })
                )
                .is_err(),
            "the actual retained native owner cannot be forced through ACK"
        );
        let previous = id;
        let released = session
            .execute(
                "release_save",
                json!({
                    "save_id": previous
                }),
            )
            .unwrap();
        id = released["save_id"].as_u64().unwrap();
        assert_ne!(id, previous);
        assert!(
            session
                .execute(
                    "save_status",
                    json!({
                        "save_id": previous
                    })
                )
                .is_err()
        );
        let released = terminal(&mut session, id);
        assert_eq!(released["outcome"]["kind"], "released");
    }
    let acknowledged = session
        .execute(
            "acknowledge_save",
            json!({
                "save_id": id
            }),
        )
        .unwrap();
    assert_eq!(acknowledged["replacement"], false);

    FailedInsert {
        directory,
        path,
        session,
        original,
        changed,
        revision,
        old_review,
        id,
    }
}

#[test]
fn failed_insert_reopens_only_actual_original_then_requires_fresh_inputs_and_review() {
    let FailedInsert {
        directory: _directory,
        path,
        mut session,
        original,
        changed,
        revision,
        old_review,
        id,
    } = failed_insert();
    let status = session.execute("session_status", json!({})).unwrap();
    assert_eq!(status["insert_retry"]["save_id"], id);
    assert_eq!(status["insert_retry"]["original_retry_eligible"], true);
    assert_eq!(status["native_save_uncertain"], false);
    if status["source_refresh_required"] == true {
        assert!(
            session
                .execute(
                    "begin_insert",
                    json!({
                        "document_revision": revision,
                        "table": "Items"
                    })
                )
                .is_err(),
            "ordinary work cannot bypass exact original-image verification"
        );
    }
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id + 1000
                })
            )
            .is_err()
    );
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id
                })
            )
            .is_err(),
        "changed current bytes cannot pass the original witness"
    );
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(session.insert_retry.is_some());
    std::fs::write(&path, &original).unwrap();
    let continued = session
        .execute(
            "retry_insert",
            json!({
                "save_id": id
            }),
        )
        .unwrap();
    assert_eq!(continued["original_verified"], true);
    assert_eq!(continued["native_write"], false);
    let status = session.execute("session_status", json!({})).unwrap();
    assert_eq!(status["source_refresh_required"], false);
    assert_eq!(status["native_save_uncertain"], false);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "retrying only reopens and prepares a new draft"
    );
    let fresh = session.revision.unwrap();
    assert_ne!(fresh, revision);
    assert!(
        session
            .execute(
                "apply_insert",
                json!({
                    "review_id": old_review
                })
            )
            .is_err()
    );
    let review = session
        .execute(
            "review_insert",
            json!({
                "document_revision": fresh,
                "draft_id": continued["draft"]["draft_id"],
                "inputs": [{
                    "text": "4"
                },
                {
                    "text": "99"
                }]
            }),
        )
        .unwrap();
    let id = save(&mut session, "apply_insert", review);
    acknowledge(&mut session, id, &path);
    let rows = session
        .execute(
            "read_rows",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Items",
                "offset": 0
            }),
        )
        .unwrap();
    assert_eq!(rows["rows"].as_array().unwrap().len(), 2);
    assert_eq!(
        rows["rows"][1][1]["text"], "99",
        "the failed operation's 50 is never replayed"
    );
    assert!(session.insert_retry.is_none());
}

#[test]
fn committed_insert_never_acquires_retry_permission_and_context_discard_changes_no_bytes() {
    let original = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let (_directory, path, mut session) = opened(&original);
    let revision = session.revision.unwrap();
    let draft = session
        .execute(
            "begin_insert",
            json!({
                "document_revision": revision,
                "table": "Items"
            }),
        )
        .unwrap();
    let review = session
        .execute(
            "review_insert",
            json!({
                "document_revision": revision,
                "draft_id": draft["draft_id"],
                "inputs": [{
                    "text": "4"
                },
                {
                    "text": "50"
                }]
            }),
        )
        .unwrap();
    let id = save(&mut session, "apply_insert", review);
    let status = session.execute("session_status", json!({})).unwrap();
    assert_eq!(status["insert_retry"]["original_retry_eligible"], false);
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id
                })
            )
            .is_err()
    );
    assert!(
        session
            .execute(
                "discard_insert_retry",
                json!({
                    "save_id": id
                })
            )
            .is_err(),
        "discard does not release the actual native actor"
    );
    acknowledge(&mut session, id, &path);
    assert!(session.insert_retry.is_none());
    let actual = std::fs::read(&path).unwrap();
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id
                })
            )
            .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), actual);
}

#[test]
fn discarding_acknowledged_retry_context_releases_only_its_advisory_witness() {
    let FailedInsert {
        directory: _directory,
        path,
        mut session,
        changed,
        id,
        ..
    } = failed_insert();
    assert!(
        session
            .execute(
                "discard_insert_retry",
                json!({
                    "save_id": id + 1000
                })
            )
            .is_err()
    );
    let result = session
        .execute(
            "discard_insert_retry",
            json!({
                "save_id": id
            }),
        )
        .unwrap();
    assert_eq!(result["discarded"], true);
    assert_eq!(result["native_write"], false);
    assert!(session.insert_retry.is_none());
    assert!(
        session
            .execute(
                "retry_insert",
                json!({
                    "save_id": id
                })
            )
            .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), changed);
}
