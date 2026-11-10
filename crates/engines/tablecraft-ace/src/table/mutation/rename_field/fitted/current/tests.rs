//! Current metadata is proved without erasing previous names or row history.
use super::*;
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_initial_table_creation::deletes,
    saved_field_rename::fitting::{self, SourceCase},
};

const SCOPE: CatalogScope = CatalogScope::new(994);
const CASES: [SourceCase; 4] = [
    SourceCase::NumberFirst,
    SourceCase::NumberHistory,
    SourceCase::InlineNumberThenText,
    SourceCase::TextNumberText,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing offset: {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn repeated_names_keep_current_properties_and_retained_histories_for_number_and_text() {
    for case in CASES {
        let original = fitting::old_name(case);
        let before = fitting::source(case);
        let mut current = before.clone();
        let mut old_name = original;
        for name in ["TaxAmount", "X", "Revenue", original] {
            let expected = fitting::rename_current(&current, old_name, name);
            let facts = image(&current)
                .inspect_current_field_rename_source("Table1", old_name, &mut budget(), &|| false)
                .unwrap();
            let candidate = image(&current)
                .prepare_current_field_rename("Table1", old_name, name, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.identity(), facts.identity());
            equal_image(candidate.before_bytes(), &current);
            equal_image(candidate.after_bytes(), &expected);
            assert!(candidate.changed());
            assert_eq!(
                &candidate.after_bytes()[8 * 4096..10 * 4096],
                &before[8 * 4096..10 * 4096]
            );

            current = expected;
            old_name = name;
        }
        assert_ne!(
            current, before,
            "returning the name must not erase stale packing history"
        );
    }
}

#[test]
fn row_tombstones_counter_gaps_and_nonzero_null_backing_bytes_remain_opaque() {
    let [_, populated] = fitting::populated_number_pair("X", &[Some(99), None, Some(-7), Some(0)]);
    let deleted = deletes::long_row(&populated, 1);
    let [_, mut null_backing] = fitting::populated_number_pair("X", &[None, Some(42)]);
    let row = u16::from_le_bytes(
        null_backing[8 * 4096 + 14..8 * 4096 + 16]
            .try_into()
            .unwrap(),
    ) as usize;
    null_backing[8 * 4096 + row + 6..8 * 4096 + row + 10].copy_from_slice(&99_i32.to_le_bytes());

    for before in [deleted, null_backing] {
        let expected = fitting::rename_current(&before, "X", "Revenue");
        let candidate = image(&before)
            .prepare_current_field_rename("Table1", "X", "Revenue", &mut budget(), &|| false)
            .unwrap();
        equal_image(candidate.after_bytes(), &expected);
        assert_eq!(
            &candidate.after_bytes()[8 * 4096..10 * 4096],
            &before[8 * 4096..10 * 4096]
        );
        for offset in [16, 20, 67] {
            assert_eq!(
                &candidate.after_bytes()[7 * 4096 + offset..7 * 4096 + offset + 4],
                &before[7 * 4096 + offset..7 * 4096 + offset + 4]
            );
        }
        let reopened = image(candidate.after_bytes());
        let rows = crate::read_table(
            &mut reopened.reader(),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if rows.rows().len() == 2 {
            assert_eq!(rows.rows()[0], [Value::Int32(1), Value::Null]);
        } else {
            assert_eq!(
                rows.rows().iter().map(|row| &row[0]).collect::<Vec<_>>(),
                [&Value::Int32(1), &Value::Int32(3), &Value::Int32(4)]
            );
        }
    }
}

#[test]
fn current_family_uses_owned_capacity_without_zeroing_opaque_free_bytes() {
    let [_, grown] = fitting::pair(SourceCase::NumberFirst, "TaxAmount");
    let mut before = fitting::rename_current(&grown, "TaxAmount", "X");
    before[7 * 4096 + 3000] = 0xa5;
    before[10 * 4096 + 3000] = 0x5a;
    let expected = fitting::rename_current(&before, "X", "Revenue");
    let candidate = image(&before)
        .prepare_current_field_rename("Table1", "X", "Revenue", &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.after_bytes(), &expected);
    assert_eq!(candidate.after_bytes()[7 * 4096 + 3000], 0xa5);
    assert_eq!(candidate.after_bytes()[10 * 4096 + 3000], 0x5a);

    let same = image(&before)
        .prepare_current_field_rename("Table1", "X", "X", &mut budget(), &|| false)
        .unwrap();
    assert!(!same.changed());
    equal_image(same.after_bytes(), &before);
    let case_only = image(&before)
        .prepare_current_field_rename("Table1", "X", "x", &mut budget(), &|| false)
        .unwrap();
    assert!(case_only.changed());
    equal_image(
        case_only.after_bytes(),
        &fitting::rename_current(&before, "X", "x"),
    );
}

#[test]
fn later_text_group_retains_existing_same_width_duplicate_review_eligibility() {
    let before = fitting::source(SourceCase::InlineNumberThenText);
    let original = image(&before);
    let observed =
        current_source::observe(&original, "Table1", "Label", &mut budget(), &|| false).unwrap();
    let fitted = image(&before).inspect_fitted_field_rename_source(
        "Table1",
        "Label",
        &mut budget(),
        &|| false,
    );
    let same_width = image(&before).inspect_same_width_field_rename_source(
        "Table1",
        "Label",
        &mut budget(),
        &|| false,
    );
    let current = Layout::capture_current(&original, &observed, &mut budget(), &|| false);
    assert!(
        current.is_ok(),
        "current packing refusal: {:?}; old fitted: {fitted:?}; old same-width: {same_width:?}",
        current.err()
    );
    assert!(matches!(
        image(&before).prepare_current_field_rename(
            "Table1",
            "Label",
            "Score",
            &mut budget(),
            &|| false,
        ),
        Err(Error::DuplicateName)
    ));
}
