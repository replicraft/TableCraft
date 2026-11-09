use super::opened;
use serde_json::json;
use tablecraft_testkit::fixtures::access::{
    mutation_fixture, saved_queries::fitting_select::storage_pair,
};

#[test]
fn inspected_read_uses_the_requested_snapshot_ceiling_without_changing_source() {
    let before = mutation_fixture::database();
    for (maximum_rows, succeeds) in [(2, false), (3, true)] {
        let (_directory, path, mut session) = opened(&before);
        let revision = session.revision.unwrap();
        let inspection = session
            .execute(
                "inspect_query",
                json!({"document_revision": revision, "sql": "SELECT Count(*) AS Records FROM Items"}),
            )
            .unwrap();
        let arguments = json!({
            "document_revision": revision,
            "query_id": inspection["query_id"],
            "options": {"snapshot_rows": maximum_rows}
        });
        crate::requests::validate("run_query", &arguments).unwrap();
        let result = session.execute("run_query", arguments);
        if succeeds {
            assert_eq!(result.unwrap()["rows"][0][0]["text"], "3");
        } else {
            assert!(
                result.is_err(),
                "two rows cannot admit a three-row snapshot"
            );
        }
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn saved_read_uses_its_own_presentation_pool_without_a_write_review() {
    let [before, _] = storage_pair();
    for (bytes, succeeds) in [(1, false), (8 * 1024 * 1024, true)] {
        let (_directory, path, mut session) = opened(&before);
        let query = session.catalog["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|query| query["name"] == "Saved")
            .unwrap();
        let arguments = json!({
            "document_revision": session.revision.unwrap(),
            "object_id": query["object_id"],
            "name": "Saved",
            "options": {"presentation": {"bytes": bytes}}
        });
        crate::requests::validate("run_saved_query", &arguments).unwrap();
        let result = session.execute("run_saved_query", arguments);
        if succeeds {
            assert_eq!(result.unwrap()["rows"][0][0]["text"], "42");
        } else {
            assert!(result.is_err(), "one byte cannot present the saved result");
        }
        assert!(session.reviewed.is_none());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
