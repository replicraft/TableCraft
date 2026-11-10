use crate::session::tests::{acknowledge, opened, save};
use serde_json::json;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed as additions, canonical_table_drop, saved_field_rename,
};

#[test]
fn number_and_explicit_text_field_addition_use_real_declarations_and_independent_after() {
    for text in [false, true] {
        let before = additions::source(additions::SourceCase::IdOnly);
        let expected = if text {
            additions::append_text(&before, "Label", 80, false)
        } else {
            additions::append_number(&before, "Score")
        };
        let (_directory, path, mut session) = opened(&before);
        let revision = session.revision.unwrap();
        let inspection = session
            .execute(
                "inspect_field_addition",
                json!({
                    "document_revision": revision,
                    "table": "Table1"
                }),
            )
            .unwrap();
        assert_eq!(
            inspection[if text {
                "short_text_available"
            } else {
                "number_available"
            }],
            true
        );
        let request = if text {
            json!({
                "document_revision": revision,
                "table": "Table1",
                "column": "Label",
                "kind": "short_text",
                "max_utf16_units": 80,
                "unicode_compression": false
            })
        } else {
            json!({
                "document_revision": revision,
                "table": "Table1",
                "column": "Score",
                "kind": "number"
            })
        };
        crate::requests::validate("review_field_addition", &request).unwrap();
        let review = session.execute("review_field_addition", request).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let id = save(&mut session, "apply_field_addition", review);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        acknowledge(&mut session, id, &path);
        let metadata = session
            .execute(
                "describe_table",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "table": "Table1"
                }),
            )
            .unwrap();
        let field = metadata["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["name"] == if text { "Label" } else { "Score" })
            .unwrap();
        assert_eq!(field["native_number"], 1);
        assert_eq!(field["storage_bytes"], if text { 160 } else { 4 });
        if !text {
            let properties = field["properties"].as_array().unwrap();
            let required = properties
                .iter()
                .find(|property| property["name"] == "Required")
                .unwrap();
            assert_eq!(
                required["observation"],
                json!({"kind": "boolean", "value": false})
            );
            let default = properties
                .iter()
                .find(|property| property["name"] == "DefaultValue")
                .unwrap();
            assert_eq!(
                default["observation"],
                json!({"kind": "text", "value": "0"})
            );
        }
    }
}

#[test]
fn native_field_rename_preserves_true_identity_and_uses_the_complete_expected_image() {
    use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::SourceCase;
    for (case, original, replacement) in [
        (SourceCase::Number, "Score", "Total"),
        (SourceCase::Mixed, "Label", "Title"),
    ] {
        let [before, expected] = saved_field_rename::pair(case);
        checked_field_rename(&before, &expected, original, replacement, &[]);
    }
}

#[test]
fn fitted_field_rename_grows_and_shrinks_original_number_text_and_history_sources() {
    use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

    for case in [
        SourceCase::NumberFirst,
        SourceCase::NumberHistory,
        SourceCase::InlineNumberThenText,
        SourceCase::TextNumberText,
    ] {
        for replacement in ["TaxAmount", "X"] {
            let [before, expected] = fitting::pair(case, replacement);
            checked_field_rename(
                &before,
                &expected,
                fitting::old_name(case),
                replacement,
                &[],
            );
        }
    }
}

#[test]
fn populated_field_rename_preserves_number_null_rows_and_indexes_through_fresh_reopen() {
    use tablecraft_testkit::fixtures::access::saved_field_rename::fitting;

    for values in [&[Some(42)][..], &[Some(42), None, Some(-17)][..]] {
        for replacement in ["TaxAmount", "X", "Total", "score"] {
            let [before, expected] = fitting::populated_number_pair(replacement, values);
            checked_field_rename(&before, &expected, "Score", replacement, values);
        }
    }
}

#[test]
fn repeated_field_rename_requalifies_current_metadata_and_preserves_cumulative_history() {
    use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

    for (case, values) in [
        (SourceCase::NumberFirst, &[][..]),
        (SourceCase::NumberHistory, &[][..]),
        (SourceCase::NumberFirst, &[Some(42), None, Some(-7)][..]),
    ] {
        let mut before = if values.is_empty() {
            fitting::source(case)
        } else {
            let [before, _] = fitting::populated_number_pair("Score", values);
            before
        };
        let mut original = "Score";
        for replacement in ["TaxAmount", "X", "Revenue", "Score"] {
            let expected = fitting::rename_current(&before, original, replacement);
            checked_field_rename(&before, &expected, original, replacement, values);
            before = expected;
            original = replacement;
        }
    }
}

fn checked_field_rename(
    before: &[u8],
    expected: &[u8],
    original: &str,
    replacement: &str,
    values: &[Option<i32>],
) {
    let (_directory, path, mut session) = opened(before);
    let revision = session.revision.unwrap();
    let inspection = session
        .execute(
            "inspect_field_rename",
            json!({
                "document_revision": revision,
                "table": "Table1",
                "column": original,
                "native_number": 1
            }),
        )
        .unwrap();
    assert_eq!(inspection["source_eligible"], true);
    assert_eq!(
        inspection["same_width_ascii_utf16_units"],
        serde_json::Value::Null
    );
    assert_eq!(
        inspection["name_profile"],
        json!({"kind": "fitted_ascii", "original_units": original.len()})
    );
    let review = session
        .execute(
            "review_field_rename",
            json!({
                "document_revision": revision,
                "table": "Table1",
                "column": original,
                "native_number": 1,
                "new_name": replacement
            }),
        )
        .unwrap();
    assert_eq!(review["changed"], true);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let id = save(&mut session, "apply_field_rename", review);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    acknowledge(&mut session, id, &path);
    let metadata = session
        .execute(
            "describe_table",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Table1"
            }),
        )
        .unwrap();
    let fields = metadata["fields"].as_array().unwrap();
    assert!(!fields.iter().any(|field| field["name"] == original));
    let renamed = fields
        .iter()
        .find(|field| field["name"] == replacement)
        .unwrap();
    assert_eq!(renamed["native_number"], 1);
    let rows = session
        .execute(
            "read_rows",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Table1",
                "offset": 0
            }),
        )
        .unwrap();
    let actual = rows["rows"].as_array().unwrap();
    assert_eq!(actual.len(), values.len());
    for (position, (row, expected)) in actual.iter().zip(values).enumerate() {
        assert_eq!(row[0]["text"], (position + 1).to_string());
        assert_eq!(row[1]["semantics"]["is_null"], expected.is_none());
        if let Some(value) = expected {
            assert_eq!(row[1]["text"], value.to_string());
        }
    }
}

#[test]
fn closed_empty_table_drop_requires_actual_inspection_and_reopens_exact_catalog_tombstone() {
    for (header, name) in [(5, "Items"), (6, "雪🙂")] {
        let source = canonical_table_drop::unindexed(header, name, false);
        let (_directory, path, mut session) = opened(&source.before);
        let revision = session.revision.unwrap();
        assert!(
            session
                .execute(
                    "review_table_drop",
                    json!({
                        "document_revision": revision,
                        "table": name
                    })
                )
                .is_err()
        );
        let inspection = session
            .execute(
                "inspect_table_drop",
                json!({
                    "document_revision": revision,
                    "table": name
                }),
            )
            .unwrap();
        assert_eq!(inspection["source_eligible"], true);
        assert_eq!(inspection["declared_rows"], 0);
        let review = session
            .execute(
                "review_table_drop",
                json!({
                    "document_revision": revision,
                    "table": name
                }),
            )
            .unwrap();
        assert_eq!(review["native_id"], inspection["native_id"]);
        assert_eq!(review["declared_rows"], 0);
        assert_eq!(std::fs::read(&path).unwrap(), source.before);
        let id = save(&mut session, "apply_table_drop", review);
        assert_eq!(std::fs::read(&path).unwrap(), source.after);
        assert_eq!(
            source.before.len(),
            source.after.len(),
            "logical DROP does not claim page reclamation"
        );
        acknowledge(&mut session, id, &path);
        assert!(
            !session.catalog["tables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|table| table["name"] == name)
        );
        assert!(
            session.catalog["tables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|table| table["system"] == true)
        );
    }
}
