//! Actual native role selection and independent bounded-capacity witnesses.
use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{
        Source as NumberSource,
        mixed::{self, after_off as raw},
        repeated,
    },
    canonical_database_options, image as ace12, mutation_fixture,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1197),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn actual_database_role_selects_only_new_proof_and_four_fitting_declarations() {
    for case in [raw::SourceCase::Number, raw::SourceCase::Mixed] {
        let before = raw::source(case);
        let purpose = image(&before)
            .field_addition_source_purpose(
                "Table1",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(purpose, FieldAdditionSourcePurpose::AfterExplicitOff);
        let facts = image(&before)
            .inspect_field_addition_source("Table1", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(facts.purpose(), purpose);
        assert!(facts.number_available());
        assert!(facts.short_text_available());
        assert_eq!(facts.after_off_name_limits(), Some((Some(64), Some(64))));
        for (column, target) in [
            ("AfterOff", FieldAdditionTarget::NumberLongZero),
            (
                "LabelOff",
                FieldAdditionTarget::ShortText {
                    max_utf16_units: 255,
                    unicode_compression: true,
                },
            ),
        ] {
            image(&before)
                .prepare_created_add_field_after_off_candidate(
                    "Table1",
                    column,
                    target,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
        }
        assert!(
            image(&before)
                .check_created_field_addition_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn original_carriers_retain_old_purpose_and_one_unchanged_source_proof() {
    for (case, expected) in [
        (
            mixed::SourceCase::IdOnly,
            CreatedFieldAdditionSourcePurpose::FirstNumber,
        ),
        (
            mixed::SourceCase::InlineNumber,
            CreatedFieldAdditionSourcePurpose::FirstNumber,
        ),
        (
            mixed::SourceCase::TwoNumberHistory,
            CreatedFieldAdditionSourcePurpose::RepeatedNumber,
        ),
    ] {
        let before = mixed::source(case);
        let facts = image(&before)
            .inspect_field_addition_source("Table1", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            facts.purpose(),
            FieldAdditionSourcePurpose::Created(expected)
        );
        assert!(facts.number_available() && facts.short_text_available());
        assert_eq!(facts.after_off_name_limits(), None);
    }
}

#[test]
fn settled_n12_without_append_room_is_unavailable_under_the_unchanged_pool() {
    let mut before = repeated::source(NumberSource::IdOnly);
    for ordinal in 2..=12 {
        before = repeated::append(&before, &format!("N{ordinal}"));
    }
    let current = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let facts = image(&current)
        .inspect_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(!facts.number_available());
    assert!(!facts.short_text_available());
    assert_eq!(facts.after_off_name_limits(), Some((None, None)));
}

#[test]
fn measured_capacity_keeps_shorter_valid_names_available() {
    let mut before = repeated::source(NumberSource::IdOnly);
    for ordinal in 0..5 {
        let name = format!("N{ordinal}{}", "x".repeat(60));
        assert_eq!(name.len(), 62);
        before = repeated::append(&before, &name);
    }
    let current = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let facts = image(&current)
        .inspect_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
    // Literal independent MR2/directory model: 1019 remaining payload bytes,
    // 901 non-name bytes, two native bytes for each ASCII name unit.
    assert_eq!(facts.after_off_name_limits().unwrap().0, Some(59));
    assert!(facts.number_available());
    let name = "Z".repeat(59);
    let expected = raw::append_number(&current, &name);
    let actual = image(&current)
        .prepare_created_add_field_after_off_candidate(
            "Table1",
            &name,
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(actual.after_bytes(), expected);
    assert!(
        image(&current)
            .prepare_created_add_field_after_off_candidate(
                "Table1",
                &"Z".repeat(60),
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn later_user_event_keeps_selected_purpose_but_fails_complete_settled_proof() {
    let before = raw::source(raw::SourceCase::Number);
    let after = raw::append_number(&before, "AfterOff");
    assert_eq!(
        image(&after)
            .field_addition_source_purpose(
                "Table1",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false
            )
            .unwrap(),
        FieldAdditionSourcePurpose::AfterExplicitOff
    );
    assert!(matches!(
        image(&after).inspect_field_addition_source("Table1", &mut budget(), &|| false),
        Err(FieldAdditionSourceError::Source)
    ));
}

#[test]
fn unsupported_formats_and_failed_observation_preserve_error_identity() {
    for before in [mutation_fixture::database(), ace12(false, false)] {
        assert!(matches!(
            image(&before).field_addition_source_purpose(
                "Missing",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false
            ),
            Err(FieldAdditionSourceError::Source)
        ));
    }
    let before = raw::source(raw::SourceCase::Number);
    let source = image(&before);
    assert!(matches!(
        source.field_addition_source_purpose(
            "Table1",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| true
        ),
        Err(FieldAdditionSourceError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        source.field_addition_source_purpose(
            "Table1",
            FieldAdditionTarget::NumberLongZero,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(FieldAdditionSourceError::Inspect(InspectError::Limit(_)))
    ));
}
