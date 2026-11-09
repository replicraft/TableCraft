//! Constructive prefix declarations stay distinct from closed appended-field history.
use super::*;
use crate::{CreateColumn, create_ace16_database};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_initial_table_creation as initial;

#[test]
fn raw_scalar_prefixes_retain_all_old_fields_through_text_then_number() {
    for case in initial::CASES {
        let source = initial::source(case);
        let untouched = source.bytes.clone();
        let expected_text = raw::append_text(&source.bytes, "Label", 80, true);
        let candidate = image(&source.bytes)
            .prepare_created_add_field_candidate(
                "Items",
                "Label",
                text(80, true),
                &mut budget(),
                &|| false,
            )
            .unwrap();

        assert_eq!(candidate.before_bytes(), untouched, "{case:?}");
        assert_eq!(candidate.after_bytes(), expected_text, "{case:?}");
        assert_eq!(source.bytes, untouched);

        let expected_number = raw::append_number(&expected_text, "AfterLabel");
        let candidate = image(&expected_text)
            .prepare_created_add_field_candidate(
                "Items",
                "AfterLabel",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false,
            )
            .unwrap();

        assert_eq!(candidate.after_bytes(), expected_number, "{case:?}");
        assert_eq!(candidate.column_count(), source.columns.len() + 2);
    }
}

#[test]
fn inert_prefix_policies_survive_but_cannot_be_invented_in_added_history() {
    let id = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let default = Value::Text("Prefix 空".into());
    let label = CreateColumn::short_text("Label", 80)
        .unwrap()
        .with_required(true)
        .with_allow_zero_length(false)
        .unwrap()
        .with_default_literal(&default)
        .unwrap()
        .with_description("Preserve this description")
        .unwrap();
    let source =
        create_ace16_database("Table1", &[id, label], &[], &mut budget(), &|| false).unwrap();
    let expected = raw::append_text(source.as_bytes(), "OtherLabel", 255, true);
    let candidate = image(source.as_bytes())
        .prepare_created_add_field_candidate(
            "Table1",
            "OtherLabel",
            text(255, true),
            &mut budget(),
            &|| false,
        )
        .unwrap();

    assert_eq!(candidate.before_bytes(), source.as_bytes());
    assert_eq!(candidate.after_bytes(), expected);
    let expected_number = raw::append_number(&expected, "AfterLabel");
    let candidate = image(&expected)
        .prepare_created_add_field_candidate(
            "Table1",
            "AfterLabel",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(candidate.after_bytes(), expected_number);

    // Literal independent oracle: latest first LVAL body ends at page10's end.
    // Its final two Boolean entries are Required(false), AllowZeroLength(true).
    let end = 11 * 4096;
    assert_eq!(
        &expected[end - 18..end],
        &[9, 0, 1, 1, 0, 0, 1, 0, 0, 9, 0, 1, 1, 1, 0, 1, 0, 255]
    );
    let mut changed = expected;
    changed[end - 10] = 255;
    assert!(
        image(&changed)
            .prepare_created_add_field_candidate(
                "Table1",
                "AfterLabel",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
}
