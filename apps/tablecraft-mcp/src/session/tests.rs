//! Real local-file adapter flows reuse independent subsystem fixtures.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tablecraft_testkit::fixtures::access::{
    canonical_catalog_names, canonical_creation, canonical_native17_creation, mutation_fixture,
    query_expression_edits,
};

pub(super) fn opened(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, Session) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.accdb");
    std::fs::write(&path, bytes).unwrap();
    let mut session = Session::new().unwrap();
    session
        .execute("open_database", json!({"path": path}))
        .unwrap();
    (directory, path, session)
}

pub(super) fn terminal(session: &mut Session, id: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let report = session
            .execute("save_status", json!({"save_id": id}))
            .unwrap();
        if report["status"] == "terminal" {
            return report;
        }
        assert!(Instant::now() < deadline, "save did not finish");
        thread::sleep(READ_POLL_INTERVAL);
    }
}

pub(super) fn save(session: &mut Session, operation: &str, review: Value) -> u64 {
    let review_id = review["review_id"].as_u64().unwrap();
    let started = session
        .execute(operation, json!({"review_id": review_id}))
        .unwrap();
    let id = started["save_id"].as_u64().unwrap();
    assert!(
        session
            .execute(operation, json!({"review_id": review_id}))
            .is_err(),
        "consumed handles cannot replay"
    );
    let report = terminal(session, id);
    assert_eq!(report["outcome"]["kind"], "saved", "{report}");
    id
}

pub(super) fn acknowledge(session: &mut Session, id: u64, path: &Path) {
    assert!(path.is_file());
    let completed = session
        .execute("acknowledge_save", json!({"save_id": id}))
        .unwrap();
    assert_eq!(completed["reopened"], true);
    assert!(session.active_save.is_none());
    assert!(!session.saves.is_busy());
}

#[test]
fn real_row_edit_insert_delete_use_actual_reviews_and_fresh_document_revisions() {
    let (_directory, path, mut session) = opened(&mutation_fixture::database());
    let old_revision = session.revision.unwrap();
    let draft = session.execute("begin_edit", json!({"document_revision": old_revision, "table": "Items", "column": "Score", "parent_ordinal": 1})).unwrap();
    let review = session.execute("review_edit", json!({"document_revision": old_revision, "draft_id": draft["draft_id"], "text": "77", "is_null": false})).unwrap();
    assert!(
        session
            .execute("apply_delete", json!({"review_id": review["review_id"]}))
            .is_err()
    );
    let id = save(&mut session, "apply_edit", review);
    acknowledge(&mut session, id, &path);
    let revision = session.revision.unwrap();
    assert_ne!(revision, old_revision);
    let page = session
        .execute(
            "read_rows",
            json!({"document_revision": revision, "table": "Items", "offset": 0}),
        )
        .unwrap();
    assert_eq!(page["rows"][1][1]["text"], "77");
    assert!(
        session
            .execute(
                "read_rows",
                json!({"document_revision": old_revision, "table": "Items", "offset": 0})
            )
            .is_err()
    );

    let source = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let (_directory, path, mut session) = opened(&source);
    let revision = session.revision.unwrap();
    let draft = session
        .execute(
            "begin_insert",
            json!({"document_revision": revision, "table": "Items"}),
        )
        .unwrap();
    let review = session.execute("review_insert", json!({"document_revision": revision, "draft_id": draft["draft_id"], "inputs": [{"text": "4", "is_null": false}, {"text": "50", "is_null": false}]})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), source, "review is inert");
    let id = save(&mut session, "apply_insert", review);
    acknowledge(&mut session, id, &path);
    let page = session
        .execute(
            "read_rows",
            json!({"document_revision": session.revision.unwrap(), "table": "Items", "offset": 0}),
        )
        .unwrap();
    assert_eq!(page["rows"].as_array().unwrap().len(), 2);

    let source = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Extrema).bytes,
    );
    let (_directory, path, mut session) = opened(&source);
    let review = session.execute("review_delete", json!({"document_revision": session.revision.unwrap(), "table": "Items", "parent_ordinal": 1})).unwrap();
    let id = save(&mut session, "apply_delete", review);
    acknowledge(&mut session, id, &path);
    let page = session
        .execute(
            "read_rows",
            json!({"document_revision": session.revision.unwrap(), "table": "Items", "offset": 0}),
        )
        .unwrap();
    assert_eq!(page["rows"].as_array().unwrap().len(), 1);
}

#[test]
fn table_and_field_schema_saves_reopen_exact_independently_constructed_images() {
    let source = canonical_catalog_names::rename_pair(6, "Items", "Customers", true);
    let (_directory, path, mut session) = opened(&source.before);
    let review = session.execute("review_table_rename", json!({"document_revision": session.revision.unwrap(), "table": "Items", "new_name": "Customers"})).unwrap();
    let id = save(&mut session, "apply_table_rename", review);
    assert_eq!(std::fs::read(&path).unwrap(), source.after);
    acknowledge(&mut session, id, &path);
    let user = session.catalog["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|table| table["name"] == "Customers")
        .unwrap();
    assert_eq!(user["system"], false);
    assert_eq!(user["local"], true);

    use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;
    let before = raw::source(raw::SourceCase::NumberHistory);
    let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let inspection = session.execute("inspect_field_drop", json!({"document_revision": revision, "table": "Table1", "column": "Score", "native_number": 1})).unwrap();
    assert_eq!(inspection["source_eligible"], true);
    let review = session.execute("review_field_drop", json!({"document_revision": revision, "table": "Table1", "column": "Score", "native_number": 1})).unwrap();
    let id = save(&mut session, "apply_field_drop", review);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    acknowledge(&mut session, id, &path);
}

#[test]
fn saved_sql_expected_image_failure_retains_acknowledged_witness_for_explicit_retry() {
    let before = query_expression_edits::two_projections();
    let expected = query_expression_edits::replace_at(
        &query_expression_edits::replace_at(&before, 3, "1000"),
        4,
        "7000",
    );
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
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let mut changed = expected.clone();
    changed[4095] ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(
        session
            .execute("acknowledge_save", json!({"save_id": id}))
            .is_err()
    );
    assert!(session.active_save.as_ref().unwrap().acknowledged);
    assert!(!session.saves.is_busy());
    assert_eq!(session.revision, Some(revision));
    std::fs::write(&path, &expected).unwrap();
    acknowledge(&mut session, id, &path);
    let result = session.execute("run_saved_query", json!({"document_revision": session.revision.unwrap(), "object_id": object_id, "name": name})).unwrap();
    assert_eq!(result["rows"][0][0]["text"], "1000");
    assert_eq!(result["rows"][0][1]["text"], "7000");
}

#[test]
fn reopening_a_document_retires_old_review_handles() {
    let (_directory, path, mut session) = opened(&mutation_fixture::database());
    let revision = session.revision.unwrap();
    let draft = session.execute("begin_edit", json!({"document_revision": revision, "table": "Items", "column": "Score", "parent_ordinal": 0})).unwrap();
    let review = session.execute("review_edit", json!({"document_revision": revision, "draft_id": draft["draft_id"], "text": "30", "is_null": false})).unwrap();
    session
        .execute("open_database", json!({"path": path}))
        .unwrap();
    assert!(
        session
            .execute("apply_edit", json!({"review_id": review["review_id"]}))
            .is_err()
    );
}

#[test]
fn blank_creation_and_table_creation_use_distinct_report_and_expected_image_owners() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fresh.accdb");
    let mut session = Session::new().unwrap();
    let started = session
        .execute("create_database", json!({"path": path}))
        .unwrap();
    let id = started["creation_id"].as_u64().unwrap();
    assert!(
        session
            .execute("creation_status", json!({"creation_id": id + 1}))
            .is_err()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let report = session
            .execute("creation_status", json!({"creation_id": id}))
            .unwrap();
        if report["status"] != "pending" {
            assert_eq!(report["status"], "created", "{report}");
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(READ_POLL_INTERVAL);
    }
    assert!(
        session
            .execute("open_created_database", json!({"creation_id": id}))
            .is_err()
    );
    session
        .execute("acknowledge_creation", json!({"creation_id": id}))
        .unwrap();
    session
        .execute("open_created_database", json!({"creation_id": id}))
        .unwrap();
    let review = session
        .execute(
            "review_table_creation",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Table1",
                "initial_autonumber": true,
                "fields": [{"name": "Score", "kind": "number"}]
            }),
        )
        .unwrap();
    let id = save(&mut session, "apply_table_creation", review);
    acknowledge(&mut session, id, &path);
    let schema = session
        .execute(
            "describe_table",
            json!({"document_revision": session.revision.unwrap(), "table": "Table1"}),
        )
        .unwrap();
    assert_eq!(schema["fields"].as_array().unwrap().len(), 2);
    assert_eq!(schema["fields"][0]["name"], "ID");
    assert_eq!(schema["fields"][1]["name"], "Score");
}

#[test]
fn refused_apply_preserves_the_actual_unconsumed_capability() {
    use super::reviewed::Reviewed;
    use tablecraft_runtime::journal_path;

    let (_directory, path, mut session) = opened(&mutation_fixture::database());
    let revision = session.revision.unwrap();
    let draft = session.execute("begin_edit", json!({"document_revision": revision, "table": "Items", "column": "Score", "parent_ordinal": 0})).unwrap();
    session.execute("review_edit", json!({"document_revision": revision, "draft_id": draft["draft_id"], "text": "77", "is_null": false})).unwrap();
    let (_, first) = session.reviewed.take().unwrap();
    let draft = session.execute("begin_edit", json!({"document_revision": revision, "table": "Items", "column": "Score", "parent_ordinal": 1})).unwrap();
    let second = session.execute("review_edit", json!({"document_revision": revision, "draft_id": draft["draft_id"], "text": "88", "is_null": false})).unwrap();
    let Reviewed::Edit(first) = first else {
        panic!("expected the genuine first cell-edit capability");
    };
    // Deliberately occupy the actual worker outside the protocol owner to
    // exercise refusal; the runtime's retained report keeps it busy until ACK.
    let id = session.saves.start(first, journal_path(&path)).unwrap();
    assert!(
        session
            .execute("apply_edit", json!({"review_id": second["review_id"]}))
            .is_err()
    );
    assert_eq!(
        session.reviewed.as_ref().unwrap().0,
        second["review_id"].as_u64().unwrap()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while session.saves.poll().is_none() {
        assert!(Instant::now() < deadline);
        thread::sleep(READ_POLL_INTERVAL);
    }
    assert!(session.saves.acknowledge(id));
    session
        .execute("open_database", json!({"path": path}))
        .unwrap();
    assert!(
        session.reviewed.is_none(),
        "fresh opening retires the now-stale capability"
    );
}

#[test]
fn nullable_optional_wire_fields_match_session_defaults_without_weakening_required_fields() {
    let (_directory, path, mut session) = opened(&mutation_fixture::database());
    let arguments = json!({"path": path, "password": null});
    crate::requests::validate("open_database", &arguments).unwrap();
    session.execute("open_database", arguments).unwrap();
    let revision = session.revision.unwrap();
    let inspected = session
        .execute(
            "inspect_query",
            json!({"document_revision": revision, "sql": "SELECT 42 AS Answer;"}),
        )
        .unwrap();
    let arguments = json!({"document_revision": revision, "query_id": inspected["query_id"], "parameters": null});
    crate::requests::validate("run_query", &arguments).unwrap();
    let result = session.execute("run_query", arguments).unwrap();
    assert_eq!(result["rows"][0][0]["text"], "42");
    let arguments = json!({"document_revision": revision, "table": "Items", "filter_id": null, "enabled": null, "sort": null});
    crate::requests::validate("sort_view", &arguments).unwrap();
    let result = session.execute("sort_view", arguments).unwrap();
    assert_eq!(result["filter_count"], 0);
    assert!(
        session
            .execute(
                "review_edit",
                json!({"document_revision": revision, "draft_id": null, "text": "42"})
            )
            .is_err()
    );
    assert!(session.execute("review_field_addition", json!({"document_revision": revision, "table": "Items", "column": "NewNumber", "kind": "number", "max_utf16_units": 80})).is_err());
}
