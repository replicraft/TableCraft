use crate::session::tests::{acknowledge, opened, save};
use serde_json::json;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_defaults as defaults, canonical_native17_creation,
    mutation_fixture,
};

#[test]
fn parameterized_sql_insert_consumes_review_once_preserving_omission_and_explicit_null() {
    for supplied_null in [false, true] {
        let source = defaults::source(defaults::Case::Seven);
        let (expected, _) = defaults::inserted(&source, supplied_null).unwrap();
        let (_directory, path, mut session) = opened(&source.bytes);
        let revision = session.revision.unwrap();
        let sql = if supplied_null {
            "PARAMETERS new_id LONG; INSERT INTO Items (Id,Score) VALUES (new_id,NULL)"
        } else {
            "PARAMETERS new_id LONG; INSERT INTO Items (Id) VALUES (new_id)"
        };
        let inspected = session
            .execute(
                "inspect_query",
                json!({
                    "document_revision": revision,
                    "sql": sql
                }),
            )
            .unwrap();
        let review = session
            .execute(
                "review_query_insert",
                json!({
                    "document_revision": revision,
                    "query_id": inspected["query_id"],
                    "parameters": [{
                        "name": "new_id",
                        "kind": "int32",
                        "text": if supplied_null {
                            "25"
                        } else {
                            "24"
                        },
                        "is_null": false
                    }]
                }),
            )
            .unwrap();
        assert_eq!(review["inserted_rows"], 1);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            source.bytes,
            "review is inert"
        );
        assert!(
            session
                .execute(
                    "apply_query_delete",
                    json!({
                        "review_id": review["review_id"]
                    })
                )
                .is_err()
        );
        let id = save(&mut session, "apply_query_insert", review);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
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
        let inserted = rows["rows"].as_array().unwrap().last().unwrap();
        assert_eq!(inserted[1]["semantics"]["is_null"], supplied_null);
        if !supplied_null {
            assert_eq!(inserted[1]["text"], "7");
        }
    }
}

#[test]
fn parameterized_sql_update_saves_independent_after_and_retires_inspection_handles() {
    let before = mutation_fixture::with_rows(&[(1, 10), (2, 20), (3, 30)]);
    let expected = mutation_fixture::with_rows(&[(1, 10), (2, 25), (3, 35)]);
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let inspected = session.execute("inspect_query", json!({
        "document_revision": revision,
        "sql": "PARAMETERS minimum LONG, increment LONG; UPDATE Items SET Score=Score+increment WHERE Id>=minimum"
    })).unwrap();
    let review = session
        .execute(
            "review_query_update",
            json!({
                "document_revision": revision,
                "query_id": inspected["query_id"],
                "parameters": [{
                    "name": "minimum",
                    "kind": "int32",
                    "text": "2"
                },
                {
                    "name": "increment",
                    "kind": "int32",
                    "text": "5"
                }]
            }),
        )
        .unwrap();
    assert_eq!(review["matched_rows"], 2);
    assert_eq!(review["changed_rows"], 2);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let id = save(&mut session, "apply_query_update", review);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    acknowledge(&mut session, id, &path);
    assert!(
        session
            .execute(
                "review_query_update",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "query_id": inspected["query_id"]
                })
            )
            .is_err()
    );
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
    assert_eq!(rows["rows"][1][1]["text"], "25");
    assert_eq!(rows["rows"][2][1]["text"], "35");
}

#[test]
fn parameterized_sql_delete_reviews_actual_count_then_reopens_remaining_membership() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let inspected = session
        .execute(
            "inspect_query",
            json!({
                "document_revision": revision,
                "sql": "PARAMETERS minimum LONG; DELETE FROM Items WHERE Id>=minimum"
            }),
        )
        .unwrap();
    let review = session
        .execute(
            "review_query_delete",
            json!({
                "document_revision": revision,
                "query_id": inspected["query_id"],
                "parameters": [{
                    "name": "minimum",
                    "kind": "int32",
                    "text": "63"
                }]
            }),
        )
        .unwrap();
    assert_eq!(review["matched_rows"], 2);
    assert_eq!(review["deleted_rows"], 2);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let id = save(&mut session, "apply_query_delete", review);
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
    assert_eq!(rows["rows"].as_array().unwrap().len(), 62);
    assert_eq!(rows["rows"][0][0]["text"], "1");
    assert_eq!(rows["rows"][61][0]["text"], "62");
    assert_eq!(rows["has_more"], false);
}
