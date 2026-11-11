//! MCP retains the actual typed DROP review and schema-only publication outcome.
use crate::session::tests::{acknowledge, opened, save, terminal};
use serde_json::json;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, field_changes, fields,
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn typed_decimal_tail_drop_inspects_reviews_applies_and_reopens_only_schema_effects() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = exact_values::append_table(&seed, "ExactValues");
    let before = exact_values::append(
        &empty,
        definition,
        Some([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]),
        Some(90_071_992_547_409_931_234),
    );
    let after = field_changes::drop_tail(&before, definition, root, "Amount");
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let original = session
        .execute(
            "read_rows",
            json!({"document_revision": revision, "table": "ExactValues", "offset": 0}),
        )
        .unwrap();
    let request = json!({
        "document_revision": revision,
        "table": "ExactValues",
        "column": "Amount",
        "native_number": 3
    });
    crate::requests::validate("inspect_field_drop", &request).unwrap();
    let inspection = session
        .execute("inspect_field_drop", request.clone())
        .unwrap();
    assert_eq!(inspection["source_eligible"], true);
    assert_eq!(inspection["column"], "Amount");
    assert_eq!(inspection["native_number"], 3);
    crate::requests::validate("review_field_drop", &request).unwrap();
    let review = session.execute("review_field_drop", request).unwrap();
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let id = save(&mut session, "apply_field_drop", review);
    let report = terminal(&mut session, id);
    let receipt = &report["outcome"]["receipt"];
    assert_eq!(receipt["schema_change"], "ColumnDropped");
    for key in [
        "matched_rows",
        "changed_rows",
        "query_change",
        "database_options_change",
    ] {
        assert!(receipt[key].is_null(), "unexpected {key}: {receipt}");
    }
    assert_image(&std::fs::read(&path).unwrap(), &after);
    acknowledge(&mut session, id, &path);
    let fresh = session.revision.unwrap();
    assert_ne!(fresh, revision);
    let page = session
        .execute(
            "read_rows",
            json!({"document_revision": fresh, "table": "ExactValues", "offset": 0}),
        )
        .unwrap();
    assert_eq!(page["columns"].as_array().unwrap().len(), 3);
    for (number, name) in ["ID", "Score", "Token"].into_iter().enumerate() {
        assert_eq!(page["columns"][number]["name"], name);
        assert_eq!(page["columns"][number]["native_number"], number);
        assert_eq!(page["rows"][0][number], original["rows"][0][number]);
    }
    assert_eq!(page["rows"].as_array().unwrap().len(), 1);
    assert_eq!(page["has_more"], false);
    assert!(
        session
            .execute(
                "read_rows",
                json!({"document_revision": revision, "table": "ExactValues", "offset": 0}),
            )
            .is_err()
    );
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}
