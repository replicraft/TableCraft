//! A typed protocol declaration composes the same native review and save actor.
use crate::session::tests::{acknowledge, opened, save, terminal};
use serde_json::json;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, typed_fields};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn retained_currency_add_uses_actual_type_advice_and_schema_only_save() {
    let source = fields::source(fields::Case::FirstNumber);
    let expected = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let (_directory, path, mut session) = opened(&source.bytes);
    let revision = session.revision.unwrap();
    let original = session
        .execute(
            "read_rows",
            json!({"document_revision":revision,"table":"Table1","offset":0}),
        )
        .unwrap();
    let available = session
        .execute(
            "inspect_field_addition",
            json!({"document_revision":revision,"table":"Table1"}),
        )
        .unwrap();
    assert!(
        available["typed_declarations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|facts| facts["scalar_type"] == "Currency"
                && facts["maximum_name_utf16_units"].as_u64().unwrap() > 0)
    );
    let arguments =
        json!({"document_revision":revision,"table":"Table1","column":"Amount","kind":"currency"});
    crate::requests::validate("review_field_addition", &arguments).unwrap();
    let review = session.execute("review_field_addition", arguments).unwrap();
    assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
    let id = save(&mut session, "apply_field_addition", review);
    let report = terminal(&mut session, id);
    let receipt = &report["outcome"]["receipt"];
    assert_eq!(receipt["schema_change"], "ColumnAdded");
    assert!(receipt["matched_rows"].is_null());
    assert!(receipt["changed_rows"].is_null());
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    acknowledge(&mut session, id, &path);
    let page = session
        .execute(
            "read_rows",
            json!({"document_revision":session.revision.unwrap(),"table":"Table1","offset":0}),
        )
        .unwrap();
    assert_eq!(page["columns"][2]["name"], "Amount");
    assert_eq!(page["columns"][2]["native_number"], 2);
    assert_eq!(
        page["rows"].as_array().unwrap().len(),
        original["rows"].as_array().unwrap().len()
    );
    for (after, before) in page["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(original["rows"].as_array().unwrap())
    {
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1], before[1]);
        assert_eq!(after[2]["semantics"]["is_null"], true);
    }
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}
