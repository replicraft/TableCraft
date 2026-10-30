//! Independent typed Text expectations; not a complete raw-image oracle.
use super::{budget, image, rows, source};
use crate::engineering::{ScalarUpdateMode, sql};
use tablecraft_core::values::Value;
use tablecraft_sql::{ParseLimits, parse_query};

const EXPECTED_LABELS: [&str; 4] = ["AX", "BX", "AX", "CX"];

#[test]
fn source_text_rows_and_typed_function_results_are_fixed_before_function_api() {
    super::qualify_sql_engineering_source_before_new_editor_api();
    let before = source();
    let original = rows(&before);
    assert_eq!(original.len(), 4);
    for (position, label) in ["A", "B", "A", "C"].iter().enumerate() {
        assert_eq!(original[position][1], Value::Text((*label).to_owned()));
    }
}

#[test]
fn trim_and_substring_update_text_without_changing_other_original_fields() {
    let before = source();
    let original = rows(&before);
    let parsed = parse_query(
        "UPDATE Items SET Label=Left(Trim(Label & 'X  '),2)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = sql::preview_batch_update(
        image(&before),
        parsed.query(),
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.matched_rows(), 4);
    let after = rows(plan.into_backend().candidate_bytes());
    for (position, expected) in EXPECTED_LABELS.iter().enumerate() {
        assert_eq!(after[position][1], Value::Text((*expected).to_owned()));
        for column in [0, 2, 3] {
            assert_eq!(after[position][column], original[position][column]);
        }
    }
}
