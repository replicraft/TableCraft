//! Protocol round trips preserve totals and actual SQL-owned declarations.
use super::*;
use crate::session::tests::opened;
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn returned_totals_design_preserves_exact_declared_inputs_when_supplied_to_build() {
    let source = mutation_fixture::database();
    let (_directory, path, mut session) = opened(&source);
    let revision = session.revision.unwrap();
    let imported = session.execute("import_query_design", json!({
        "document_revision": revision,
        "sql": "PARAMETERS [Minimum] LONG; SELECT Score,Sum(Id) AS Total FROM Items WHERE Id>1 GROUP BY Score HAVING Sum(Id)>=[Minimum]"
    })).unwrap();
    assert_eq!(imported["draft"]["totals"], true);
    assert!(
        imported["draft"]["parameters_sql"]
            .as_str()
            .unwrap()
            .contains("Minimum")
    );
    assert!(
        imported["draft"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|column| column["total"] == "sum")
    );
    let built = session
        .execute(
            "build_query_design",
            json!({
                "document_revision": revision, "draft": imported["draft"]
            }),
        )
        .unwrap();
    assert_eq!(built["sql"], imported["sql"]);
    assert_eq!(built["draft"], imported["draft"]);
    assert!(!session.saves.is_busy());
    assert_eq!(std::fs::read(path).unwrap(), source);
}

#[test]
fn parameters_header_cannot_hide_a_statement_in_an_external_draft() {
    let draft = json!({"sources":[],"joins":[],"columns":[],"totals":true,
        "parameters_sql":"PARAMETERS [Minimum] LONG; SELECT 1; DELETE FROM Items"});
    assert!(parse_draft(&draft).is_err());
}
