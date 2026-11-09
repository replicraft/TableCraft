use crate::session::tests::{acknowledge, opened, save};
use serde_json::json;
use tablecraft_testkit::fixtures::access::{
    attachment_modern, canonical_creator_rename, canonical_database_options, complex_links,
    saved_queries::fitting_select::storage_pair, version_history,
};

#[test]
fn database_options_inspect_review_save_and_reopen_report_explicit_off_from_real_bytes() {
    let before = canonical_creator_rename::originals().remove(0);
    let expected = canonical_database_options::explicit_off(&before);
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let inspection = session
        .execute(
            "inspect_database_options",
            json!({
                "document_revision": revision
            }),
        )
        .unwrap();
    assert_eq!(inspection["disable_available"], true);
    let review = session
        .execute(
            "review_disable_name_auto_correct",
            json!({
                "document_revision": revision
            }),
        )
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let id = save(&mut session, "apply_database_options", review);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    acknowledge(&mut session, id, &path);
    let inspection = session
        .execute(
            "inspect_database_options",
            json!({
                "document_revision": session.revision.unwrap()
            }),
        )
        .unwrap();
    assert_eq!(inspection["name_auto_correct"], "ExplicitOff");
}

#[test]
fn singular_query_expression_service_uses_its_distinct_review_and_actual_saved_read() {
    let [before, expected] = storage_pair();
    let (_directory, path, mut session) = opened(&before);
    let query = session.catalog["queries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|query| query["name"] == "Saved")
        .unwrap();
    let object_id = query["object_id"].as_str().unwrap().to_string();
    let revision = session.revision.unwrap();
    let review = session
        .execute(
            "review_query_expression",
            json!({
                "document_revision": revision,
                "object_id": object_id,
                "name": "Saved",
                "expected": "42",
                "replacement": "43"
            }),
        )
        .unwrap();
    assert_eq!(review["changed"], true);
    let id = save(&mut session, "apply_query_expression", review);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    acknowledge(&mut session, id, &path);
    let inspected = session
        .execute(
            "inspect_saved_query",
            json!({
                "document_revision": session.revision.unwrap(),
                "object_id": object_id,
                "name": "Saved"
            }),
        )
        .unwrap();
    assert_eq!(inspected["name"], "Saved");
    let result = session
        .execute(
            "run_saved_query",
            json!({
                "document_revision": session.revision.unwrap(),
                "object_id": object_id,
                "name": "Saved"
            }),
        )
        .unwrap();
    assert_eq!(result["rows"][0][0]["text"], "43");
}

#[test]
fn collection_inspection_preserves_child_kinds_and_never_opens_attachment_actions() {
    for (bytes, kind, rows) in [
        (complex_links::presence_cases(), "Multivalue", 3),
        (attachment_modern::database(6), "Attachments", 2),
        (version_history::database(), "VersionHistory", 2),
    ] {
        let (_directory, path, mut session) = opened(&bytes);
        let revision = session.revision.unwrap();
        let contents = session
            .execute(
                "inspect_collection",
                json!({
                    "document_revision": revision,
                    "table": "Items",
                    "column": "Score",
                    "parent_ordinal": 0
                }),
            )
            .unwrap();
        assert_eq!(contents["kind"], kind);
        assert_eq!(contents["assigned"], true);
        assert_eq!(contents["rows"].as_array().unwrap().len(), rows);
        assert!(
            session
                .execute(
                    "inspect_collection",
                    json!({
                        "document_revision": revision,
                        "table": "Items",
                        "column": "Id",
                        "parent_ordinal": 0
                    })
                )
                .is_err()
        );
        assert!(!session.saves.is_busy());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
