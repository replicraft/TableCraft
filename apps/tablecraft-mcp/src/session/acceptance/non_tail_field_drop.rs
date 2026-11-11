//! MCP uses the same non-tail source review and consuming schema save as GUI hosts.
use crate::session::tests::{acknowledge, opened, save, terminal};
use serde_json::json;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};

fn source() -> (fields::Source, Vec<u8>) {
    let mut source = fields::source(fields::Case::FirstNumber);
    let added = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    source.bytes = typed_fields::update_currency(&added, source.definition, 0, Some(12345));
    let after = field_changes::drop_field(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Score",
    );
    (source, after)
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn non_tail_score_drop_reviews_applies_and_reopens_currency_with_original_native_number() {
    let (source, after) = source();
    let (_directory, path, mut session) = opened(&source.bytes);
    let revision = session.revision.unwrap();
    let original = session
        .execute(
            "read_rows",
            json!({
                "document_revision": revision, "table": "Table1", "offset": 0,
            }),
        )
        .unwrap();
    let target = json!({
        "document_revision": revision, "table": "Table1",
        "column": "Score", "native_number": 1,
    });
    crate::requests::validate("inspect_field_drop", &target).unwrap();
    let inspection = session
        .execute("inspect_field_drop", target.clone())
        .unwrap();
    assert_eq!(inspection["source_eligible"], true);
    assert_eq!(inspection["native_number"], 1);
    crate::requests::validate("review_field_drop", &target).unwrap();
    let review = session.execute("review_field_drop", target).unwrap();
    assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    let id = save(&mut session, "apply_field_drop", review);
    let report = terminal(&mut session, id);
    let receipt = &report["outcome"]["receipt"];
    assert_eq!(receipt["schema_change"], "ColumnDropped");
    for name in [
        "matched_rows",
        "changed_rows",
        "query_change",
        "database_options_change",
    ] {
        assert!(receipt[name].is_null(), "unexpected receipt effect {name}");
    }
    assert_image(&std::fs::read(&path).unwrap(), &after);
    acknowledge(&mut session, id, &path);
    let fresh = session.revision.unwrap();
    assert_ne!(fresh, revision);
    let page = session
        .execute(
            "read_rows",
            json!({
                "document_revision": fresh, "table": "Table1", "offset": 0,
            }),
        )
        .unwrap();
    assert_eq!(page["columns"].as_array().unwrap().len(), 2);
    assert_eq!(page["columns"][0]["name"], "ID");
    assert_eq!(page["columns"][0]["native_number"], 0);
    assert_eq!(page["columns"][1]["name"], "Amount");
    assert_eq!(page["columns"][1]["native_number"], 2);
    assert_eq!(page["rows"].as_array().unwrap().len(), 2);
    for row in 0..2 {
        assert_eq!(page["rows"][row][0], original["rows"][row][0]);
        assert_eq!(page["rows"][row][1], original["rows"][row][2]);
    }
    assert!(
        session
            .execute(
                "read_rows",
                json!({
                    "document_revision": revision, "table": "Table1", "offset": 0,
                })
            )
            .is_err()
    );
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}

#[tokio::test(flavor = "current_thread")]
async fn readonly_mcp_actor_can_review_non_tail_drop_but_cannot_apply_its_real_capability() {
    let (source, _) = source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("readonly.accdb");
    std::fs::write(&path, &source.bytes).unwrap();
    let actor = crate::actor::Actor::start(
        directory.path().to_path_buf(),
        crate::permissions::Permissions::default(),
    )
    .unwrap();
    let opened = actor
        .execute(
            "open_database".into(),
            json!({"path": path}),
            std::future::pending(),
        )
        .await
        .unwrap();
    let revision = opened["catalog"]["document_revision"].as_u64().unwrap();
    let target = json!({"document_revision": revision, "table": "Table1",
        "column": "Score", "native_number": 1});
    let inspection = actor
        .execute(
            "inspect_field_drop".into(),
            target.clone(),
            std::future::pending(),
        )
        .await
        .unwrap();
    assert_eq!(inspection["source_eligible"], true);
    let review = actor
        .execute("review_field_drop".into(), target, std::future::pending())
        .await
        .unwrap();
    let refused = actor
        .execute(
            "apply_field_drop".into(),
            json!({"review_id": review["review_id"]}),
            std::future::pending(),
        )
        .await
        .expect_err("a real runtime review does not grant server schema-write permission");
    assert!(refused.contains("--allow-schema-write"));
    let status = actor
        .execute("session_status".into(), json!({}), std::future::pending())
        .await
        .unwrap();
    assert!(status["active_save_id"].is_null());
    assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}
