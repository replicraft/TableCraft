//! Source purpose preserves legacy Number admission; it is never a write oracle.
use super::*;
use crate::{CreateColumn, CreatedDefaultZeroColumnRequired, create_ace16_database};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed as raw, image as ace12, mutation_fixture,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(945),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn purpose(bytes: &[u8], target: FieldAdditionTarget) -> CreatedFieldAdditionSourcePurpose {
    image(bytes)
        .created_field_addition_source_purpose("Table1", target, &mut budget(), &|| false)
        .unwrap()
}

fn primary() -> CreateColumn<'static> {
    CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap()
}

#[test]
fn existing_carriers_and_actual_text_metadata_choose_one_explicit_purpose() {
    for case in [raw::SourceCase::IdOnly, raw::SourceCase::InlineNumber] {
        let bytes = raw::source(case);
        assert_eq!(
            purpose(&bytes, FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::FirstNumber
        );
    }
    for case in [
        raw::SourceCase::TwoNumberHistory,
        raw::SourceCase::ThreeNumberHistory,
    ] {
        let bytes = raw::source(case);
        assert_eq!(
            purpose(&bytes, FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::RepeatedNumber
        );
    }
    for compression in [false, true] {
        let before = raw::append_text(
            &raw::source(raw::SourceCase::IdOnly),
            "Label",
            255,
            compression,
        );
        assert_eq!(
            purpose(&before, FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::TypedField
        );
        assert_eq!(
            purpose(
                &before,
                FieldAdditionTarget::ShortText {
                    max_utf16_units: 255,
                    unicode_compression: true
                }
            ),
            CreatedFieldAdditionSourcePurpose::TypedField
        );
    }
}

#[test]
fn inert_non_number_and_no_default_prefixes_keep_existing_number_routes() {
    for companion in [
        CreateColumn::new("ByteValue", ScalarType::Byte),
        CreateColumn::new("OldLong", ScalarType::Int32),
    ] {
        let seed = create_ace16_database(
            "Table1",
            &[primary(), companion],
            &[],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            purpose(seed.as_bytes(), FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::FirstNumber
        );
        let first = image(seed.as_bytes())
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Score",
                CreatedDefaultZeroColumnRequired::StoredFalse,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            purpose(first.after_bytes(), FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::RepeatedNumber
        );
        image(first.after_bytes())
            .check_repeated_default_zero_add_column_source("Table1", &mut budget(), &|| false)
            .unwrap();
        // New typed source also has its own positive prefix proof; route choice
        // nevertheless retains the explicit legacy Number purpose above.
        image(seed.as_bytes())
            .check_created_field_addition_source("Table1", &mut budget(), &|| false)
            .unwrap();
    }
}

#[test]
fn false_azl_selects_typed_without_changing_old_no_default_or_number_languages() {
    let field = CreateColumn::short_text("OldText", 80)
        .unwrap()
        .with_required(false)
        .with_allow_zero_length(false)
        .unwrap();
    let seed = create_ace16_database("Table1", &[primary(), field], &[], &mut budget(), &|| false)
        .unwrap();
    image(seed.as_bytes())
        .prepare_created_add_int32_column_candidate("Table1", "NoDefault", &mut budget(), &|| false)
        .unwrap();
    assert!(
        image(seed.as_bytes())
            .check_created_default_zero_add_column_source("Table1", &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(
        purpose(seed.as_bytes(), FieldAdditionTarget::NumberLongZero),
        CreatedFieldAdditionSourcePurpose::TypedField
    );
    image(seed.as_bytes())
        .check_created_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
}

#[test]
fn compression_permission_selects_typed_without_inventing_azl() {
    let field = CreateColumn::short_text("OldText", 80)
        .unwrap()
        .with_unicode_compression(true)
        .unwrap();
    let seed = create_ace16_database("Table1", &[primary(), field], &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        purpose(seed.as_bytes(), FieldAdditionTarget::NumberLongZero),
        CreatedFieldAdditionSourcePurpose::TypedField
    );
    image(seed.as_bytes())
        .check_created_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
}

#[test]
fn older_formats_keep_source_refusal_before_selected_table_inspection() {
    for bytes in [mutation_fixture::database(), ace12(false, false)] {
        for target in [
            FieldAdditionTarget::NumberLongZero,
            FieldAdditionTarget::ShortText {
                max_utf16_units: 255,
                unicode_compression: true,
            },
        ] {
            assert!(matches!(
                image(&bytes).created_field_addition_source_purpose(
                    "MissingTable",
                    target,
                    &mut budget(),
                    &|| false
                ),
                Err(FieldAdditionError::Source)
            ));
        }
    }
}

#[test]
fn metadata_selection_preserves_cancellation_and_resource_failures() {
    let source = image(&raw::source(raw::SourceCase::TwoNumberHistory));
    assert!(matches!(
        source.created_field_addition_source_purpose(
            "Table1",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| true
        ),
        Err(FieldAdditionError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        source.created_field_addition_source_purpose(
            "Table1",
            FieldAdditionTarget::NumberLongZero,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(FieldAdditionError::Inspect(InspectError::Limit(_)))
    ));
}

#[test]
fn broad_inert_prefix_policies_choose_typed_before_number_qualification() {
    let text_default = Value::Text("Retain prefix".into());
    let byte_default = Value::Byte(0);
    let number_default = Value::Int32(7);
    let fields = [
        CreateColumn::new("RequiredByte", ScalarType::Byte).with_required(true),
        CreateColumn::new("DescribedByte", ScalarType::Byte)
            .with_description("Retain description")
            .unwrap(),
        CreateColumn::new("DefaultByte", ScalarType::Byte)
            .with_default_literal(&byte_default)
            .unwrap(),
        CreateColumn::new("DefaultNumber", ScalarType::Int32)
            .with_default_literal(&number_default)
            .unwrap(),
        CreateColumn::short_text("DefaultText", 80)
            .unwrap()
            .with_default_literal(&text_default)
            .unwrap(),
    ];
    for field in fields {
        let source =
            create_ace16_database("Table1", &[primary(), field], &[], &mut budget(), &|| false)
                .unwrap();
        assert_eq!(
            purpose(source.as_bytes(), FieldAdditionTarget::NumberLongZero),
            CreatedFieldAdditionSourcePurpose::TypedField
        );
        assert!(
            image(source.as_bytes())
                .check_created_default_zero_add_column_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
        image(source.as_bytes())
            .prepare_created_add_field_candidate(
                "Table1",
                "Score",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false,
            )
            .unwrap();
    }
}
