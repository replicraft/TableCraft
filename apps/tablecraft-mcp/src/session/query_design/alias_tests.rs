//! Protocol alias data reaches the same checked builder and explicit query service.
use super::*;
use crate::session::tests::opened;
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn supplied_alias_roundtrip_build_and_run_preserve_heading_source_and_other_draft_facts() {
    let before = mutation_fixture::database();
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let mut imported = session
        .execute(
            "import_query_design",
            json!({
                "document_revision":revision,"sql":"SELECT Score FROM Items ORDER BY Id"
            }),
        )
        .unwrap();
    let mut draft = imported["draft"].take();
    draft["columns"][0]["alias"] = json!("Score: view");
    let built = session
        .execute(
            "build_query_design",
            json!({
                "document_revision":revision,"draft":draft
            }),
        )
        .unwrap();
    assert_eq!(built["draft"]["columns"][0]["alias"], "Score: view");
    assert_eq!(built["draft"]["totals"], false);
    assert_eq!(built["draft"]["parameters_sql"], "");
    let inspected = session
        .execute(
            "inspect_query",
            json!({
                "document_revision":revision,"sql":built["sql"]
            }),
        )
        .unwrap();
    let result = session
        .execute(
            "run_query",
            json!({
                "document_revision":revision,"query_id":inspected["query_id"]
            }),
        )
        .unwrap();
    assert_eq!(result["columns"][0]["name"], "Score: view");
    let rows = result["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0]["text"], "10");
    assert_eq!(rows[1][0]["text"], "20");
    assert_eq!(rows[2][0]["text"], "30");
    assert!(!session.saves.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}
