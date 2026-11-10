//! Full metadata images and old-row NULL semantics are independent of the writer.
use crate::{
    CurrentFieldAdditionError, FieldAdditionSourcePurpose, FieldAdditionTarget, NativeImage,
    inspect_table_schema, read_table,
    table::mutation::current_schema::{Family, UserStorage},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields::{
    self as raw, CASES, Case, Target,
};

const SCOPE: CatalogScope = CatalogScope::new(4010);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_field_additions_match_complete_raw_images_and_preserve_historical_nulls() {
    for case in CASES {
        let source = raw::source(case);
        for target in [Target::Number, Target::ShortText] {
            let expected = raw::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target,
            );
            let native = image(&source.bytes);
            let old = read_table(
                &mut native.reader(),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let availability = image(&source.bytes)
                .inspect_field_addition_source("Table1", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                availability.purpose(),
                FieldAdditionSourcePurpose::RetainedNumber
            );
            assert!(availability.number_available());
            assert!(availability.short_text_available());
            let (observed, number, id, count) = match target {
                Target::Number => {
                    assert_eq!(
                        native
                            .field_addition_source_purpose(
                                "Table1",
                                FieldAdditionTarget::NumberLongZero,
                                &mut budget(),
                                &|| false
                            )
                            .unwrap(),
                        FieldAdditionSourcePurpose::RetainedNumber
                    );
                    let facts = native
                        .inspect_retained_field_addition_source("Table1", &mut budget(), &|| false)
                        .unwrap();
                    let candidate = native
                        .prepare_retained_number_field_addition(
                            "Table1",
                            target.name(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?}: {error:?}"));
                    assert!(candidate.is_retained());
                    assert_eq!(candidate.native_number(), facts.native_number());
                    assert_eq!(candidate.column_id(), facts.column_id());
                    equal_image(candidate.before_bytes(), &source.bytes);
                    (
                        candidate.after_bytes().to_vec(),
                        candidate.native_number(),
                        candidate.column_id(),
                        candidate.column_count(),
                    )
                }
                Target::ShortText => {
                    assert_eq!(
                        native
                            .field_addition_source_purpose(
                                "Table1",
                                FieldAdditionTarget::ShortText {
                                    max_utf16_units: 255,
                                    unicode_compression: true
                                },
                                &mut budget(),
                                &|| false
                            )
                            .unwrap(),
                        FieldAdditionSourcePurpose::RetainedShortText
                    );
                    let facts = native
                        .inspect_retained_short_text_field_addition_source(
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    let candidate = native
                        .prepare_retained_short_text_field_addition(
                            "Table1",
                            target.name(),
                            255,
                            true,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?}: {error:?}"));
                    assert!(candidate.is_retained());
                    assert_eq!(candidate.native_number(), facts.native_number());
                    assert_eq!(candidate.column_id(), facts.column_id());
                    equal_image(candidate.before_bytes(), &source.bytes);
                    (
                        candidate.after_bytes().to_vec(),
                        candidate.native_number(),
                        candidate.column_id(),
                        candidate.column_count(),
                    )
                }
            };
            equal_image(&observed, &expected);
            assert_eq!(count, old.schema().columns().len() + 1);
            let schema = inspect_table_schema(
                &mut Cursor::new(&observed),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(schema.definition_page(), source.definition);
            assert_eq!(schema.columns().last().unwrap().number(), number);
            let stored_id = source.definition as usize * 4096 + 75 + 25 * (count - 1) + 9;
            assert_eq!(
                u16::from_le_bytes(expected[stored_id..stored_id + 2].try_into().unwrap()),
                id
            );
            let current = read_table(
                &mut Cursor::new(&observed),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(current.rows().len(), old.rows().len());
            for (before, after) in old.rows().iter().zip(current.rows()) {
                assert_eq!(&after[..before.len()], before);
                assert_eq!(after.last(), Some(&Value::Null));
            }
            let other_before = read_table(
                &mut Cursor::new(&source.bytes),
                SCOPE,
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let other_after = read_table(
                &mut Cursor::new(&observed),
                SCOPE,
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(other_before.rows(), other_after.rows());
        }
    }
}

#[test]
fn retained_field_addition_preserves_current_source_closure_and_control_errors() {
    let source = raw::source(Case::FirstNumber);
    let standard = NativeImage::capture(
        &mut Cursor::new(&source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        standard
            .prepare_current_number_field_addition("Table1", "NextNumber", &mut budget(), &|| false)
            .is_err()
    );
    for name in ["", "ID", "日本語"] {
        assert!(
            image(&source.bytes)
                .prepare_retained_number_field_addition("Table1", name, &mut budget(), &|| false)
                .is_err()
        );
    }
    for maximum in [0, 256] {
        assert!(matches!(
            image(&source.bytes).prepare_retained_short_text_field_addition(
                "Table1",
                "NextText",
                maximum,
                true,
                &mut budget(),
                &|| false
            ),
            Err(CurrentFieldAdditionError::Source)
        ));
    }
    assert!(
        image(&source.bytes)
            .prepare_retained_number_field_addition("Table1", "NextNumber", &mut budget(), &|| true)
            .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(
            image(&source.bytes)
                .prepare_retained_number_field_addition(
                    "Table1",
                    "NextNumber",
                    &mut allowance,
                    &|| false
                )
                .is_err()
        );
    }
    let expected = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        Target::Number,
    );
    assert!(matches!(
        image(&expected).prepare_retained_number_field_addition(
            "Table1",
            "nextnumber",
            &mut budget(),
            &|| false
        ),
        Err(CurrentFieldAdditionError::DuplicateName)
    ));
}

#[test]
fn retained_field_addition_uses_actual_fitting_owned_property_candidates() {
    for case in raw::property_pages::CASES {
        let source = raw::property_pages::source(case);
        for target in [Target::Number, Target::ShortText] {
            let expected = raw::property_pages::after(&source, target);
            let native = image(&source.bytes);
            let observed = match target {
                Target::Number => {
                    let facts = native
                        .inspect_retained_field_addition_source("Table1", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(facts.maximum_name_utf16_units(), 64);
                    let candidate = native
                        .prepare_retained_number_field_addition(
                            "Table1",
                            target.name(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?}: {error:?}"));
                    equal_image(candidate.before_bytes(), &source.bytes);
                    candidate.after_bytes().to_vec()
                }
                Target::ShortText => {
                    let facts = native
                        .inspect_retained_short_text_field_addition_source(
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    assert_eq!(facts.maximum_name_utf16_units(), 64);
                    let candidate = native
                        .prepare_retained_short_text_field_addition(
                            "Table1",
                            target.name(),
                            255,
                            true,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?}: {error:?}"));
                    equal_image(candidate.before_bytes(), &source.bytes);
                    candidate.after_bytes().to_vec()
                }
            };
            equal_image(&observed, &expected);
            let native = image(&observed);
            let table = Family::Retained
                .observe(&native, "Table1", &mut budget(), &|| false)
                .unwrap();
            assert!(
                matches!(table.user_storage, UserStorage::Single { reference } if reference >> 8 == source.chosen_page)
            );
        }
        // A hinted page without the corresponding owned bit is corruption,
        // including when another allocated carrier could fit the new body.
        let mut corrupt = source.bytes;
        corrupt[source.owned_cell] &= !source.mask;
        corrupt[source.hint_cell] |= source.mask;
        assert!(
            image(&corrupt)
                .inspect_retained_field_addition_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
}
