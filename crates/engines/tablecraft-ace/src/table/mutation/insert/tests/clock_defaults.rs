//! Clock inputs and whole expected rows are literal before any clock-default API.
mod api;
mod authority;
mod extra_sources;
mod model;

use super::model as original;
use crate::{RuleCheckStatus, check_table_constraints, read_table};
use model::{PAGE, Reading, Row};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

fn checked(bytes: &[u8], rows: usize) {
    original::checked(bytes, rows, 2);
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    for field in report.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported),
            "field {}",
            field.name()
        );
    }
}

#[test]
fn qualify_clock_default_source_and_literal_date_now_whole_images_before_api() {
    for code in [5, 6] {
        for generated in [false, true] {
            let case = model::source(code, generated, "Date()", "Now()");
            checked(&case.before, 2);
            for reading in [
                Reading::LeapNoon,
                Reading::BeforeEpochSix,
                Reading::EpochSix,
                Reading::Minimum,
                Reading::Maximum,
                Reading::MidnightFirst,
            ] {
                let _fixed_input = reading.input();
                let added = model::added(&case, reading);
                let expected = model::expected(&case, &added);
                checked(&expected, 4);
                assert_eq!(
                    &expected[4 * PAGE..5 * PAGE],
                    &case.before[4 * PAGE..5 * PAGE]
                );
                let snapshot = read_table(
                    &mut Cursor::new(&expected),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(snapshot.rows()[2], added[0].values());
                assert_eq!(snapshot.rows()[3], added[1].values());
            }
        }
    }
}

#[test]
fn qualify_supplied_clock_fields_keep_null_alias_bits_and_exact_noop_images_before_api() {
    for code in [5, 6] {
        for generated in [false, true] {
            let case = model::source(code, generated, "Date()", "Now()");
            let row = Row {
                dated: Some(0xbfd0000000000000),
                id: if generated { 261 } else { 10 },
                stamped: None,
            };
            let expected = model::expected(&case, std::slice::from_ref(&row));
            checked(&case.before, 2);
            checked(&expected, 3);
            assert_eq!(model::expected(&case, &[]), case.before);
            let snapshot = read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(snapshot.rows()[2], row.values());
        }
    }
}

#[test]
fn qualify_clock_default_refusal_sources_without_evaluating_needed_calls() {
    for code in [5, 6] {
        for text in [
            "Timer()",
            "Time()",
            "Date(1)",
            "VBA.Now()",
            "Now$()",
            "Now",
            "UnresolvedFunction()",
        ] {
            let case = model::source(code, false, text, "Now()");
            checked(&case.before, 2);
        }
        checked(&model::wrong_target_source(code).before, 2);
    }
}

#[test]
fn qualify_clock_counter_exhaustion_source_before_host_callback() {
    for code in [5, 6] {
        let case = model::maximum_counter_source(code);
        checked(&case.before, 2);
        assert_eq!(
            &case.before[10 * PAGE + 20..10 * PAGE + 24],
            &i32::MAX.to_le_bytes()
        );
        assert_eq!(model::expected(&case, &[]), case.before);
    }
}
