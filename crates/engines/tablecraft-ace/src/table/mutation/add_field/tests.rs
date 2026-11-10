//! Full raw-image and non-backfill checks independent of native encoders.
mod readdition;
use super::*;
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    self as raw, history, populated,
};

const SCOPE: CatalogScope = CatalogScope::new(1209);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 20_000_000)
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
fn current_number_add_preserves_existing_text_and_retired_property_history() {
    for before in [
        raw::source(raw::SourceCase::Number),
        raw::source(raw::SourceCase::Mixed),
        history::source(history::SourceCase::NumberThenNumber),
        history::source(history::SourceCase::MixedThenText),
    ] {
        let expected = raw::append_number(&before, "Bonus");
        let facts = image(&before)
            .inspect_current_field_addition_source("Table1", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(facts.maximum_name_utf16_units(), 64);
        let candidate = image(&before)
            .prepare_current_number_field_addition("Table1", "Bonus", &mut budget(), &|| false)
            .unwrap();
        equal_image(candidate.before_bytes(), &before);
        equal_image(candidate.after_bytes(), &expected);
        assert_eq!(candidate.native_number(), facts.native_number());
        assert_eq!(candidate.column_id(), facts.column_id());
        assert_eq!(candidate.column_count(), facts.native_number() + 1);
    }
}

#[test]
fn populated_number_add_retains_old_records_and_projects_absent_field_as_null() {
    let [before, expected] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let candidate = image(&before)
        .prepare_current_number_field_addition("Table1", "Bonus", &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.after_bytes(), &expected);
    assert_eq!(
        &candidate.after_bytes()[8 * PAGE..10 * PAGE],
        &before[8 * PAGE..10 * PAGE]
    );

    let rows = crate::read_table(
        &mut Cursor::new(candidate.after_bytes()),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        rows.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(42), Value::Null],
            vec![Value::Int32(2), Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Int32(-7), Value::Null],
        ]
    );
}

#[test]
fn proposed_names_and_control_failures_do_not_produce_replacements() {
    let before = raw::source(raw::SourceCase::Number);
    for name in ["", "Bad.Name", "\u{e9}"] {
        assert!(matches!(
            image(&before).prepare_current_number_field_addition(
                "Table1",
                name,
                &mut budget(),
                &|| false,
            ),
            Err(CurrentFieldAdditionError::InvalidName)
        ));
    }
    assert!(matches!(
        image(&before).prepare_current_number_field_addition(
            "Table1",
            "score",
            &mut budget(),
            &|| false,
        ),
        Err(CurrentFieldAdditionError::DuplicateName)
    ));
    assert!(matches!(
        image(&before).prepare_current_number_field_addition(
            "Table1",
            &"X".repeat(65),
            &mut budget(),
            &|| false,
        ),
        Err(CurrentFieldAdditionError::InvalidName)
    ));
    assert!(matches!(
        image(&before).inspect_current_field_addition_source("Table1", &mut budget(), &|| true),
        Err(CurrentFieldAdditionError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before).inspect_current_field_addition_source(
            "Table1",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(CurrentFieldAdditionError::Inspect(InspectError::Limit(_)))
    ));
}

#[test]
fn independent_after_verification_rejects_unrelated_and_live_metadata_changes() {
    let before = raw::source(raw::SourceCase::Number);
    let candidate = image(&before)
        .prepare_current_number_field_addition("Table1", "Bonus", &mut budget(), &|| false)
        .unwrap();
    let current = candidate.proof.table();
    let plan = Plan::capture(
        candidate.proof.image(),
        &current.schema,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let body = properties::append(
        &current.properties,
        "Bonus",
        Target::Number,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let append = catalog_property::append_fitted_body(
        candidate.proof.image(),
        &body,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let descriptor = DescriptorPatch::capture(
        candidate.proof.image(),
        &current.catalog,
        current.schema.object_id(),
        append.reference,
        body.len(),
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let definition = 7 * PAGE;
    for offset in [
        8 * PAGE + 200,
        9 * PAGE + 200,
        definition + 16,
        definition + plan.descriptor_end + 9,
        descriptor.page_range().start + 100,
        candidate.after.len() - 1,
    ] {
        let mut forged = candidate.after.clone();
        forged[offset] ^= 1;
        assert!(
            verify::candidate(
                &candidate.proof,
                &plan,
                Target::Number,
                &descriptor,
                &forged,
                "Bonus",
                &body,
                append.reference,
                &mut budget(),
                &|| false,
            )
            .is_err(),
            "forgery accepted at {offset}"
        );
    }
}

#[test]
fn current_addition_availability_is_not_derived_from_empty_creator_replay() {
    use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

    let before = fitting::pair(SourceCase::NumberFirst, "X")[1].clone();
    let source = image(&before)
        .inspect_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(source.number_available());
    assert_eq!(
        source.purpose(),
        crate::FieldAdditionSourcePurpose::CurrentNumber
    );

    let [before, _] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let source = image(&before)
        .inspect_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(source.number_available());
    assert!(source.short_text_available());
}

#[test]
fn non_ascii_existing_namespace_refuses_ascii_addition_before_duplicate_matching() {
    let mut before = raw::source(raw::SourceCase::Number);
    let old = [b'S', 0, b'c', 0, b'o', 0, b'r', 0, b'e', 0];
    let locations = before
        .windows(old.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == old).then_some(offset))
        .collect::<Vec<_>>();
    // Independently change the TDEF name and corresponding live MR2 group.
    // Equal UTF-16 width preserves both native frames without any encoder.
    assert_eq!(locations.len(), 2);
    for offset in locations {
        before[offset..offset + 2].copy_from_slice(&0x212a_u16.to_le_bytes());
    }
    let captured = image(&before);
    current_schema::observe(&captured, "Table1", &mut budget(), &|| false).unwrap();
    assert!(matches!(
        captured.inspect_current_field_addition_source("Table1", &mut budget(), &|| false),
        Err(CurrentFieldAdditionError::Source)
    ));
    assert!(matches!(
        image(&before).prepare_current_number_field_addition(
            "Table1",
            "Kcore",
            &mut budget(),
            &|| false,
        ),
        Err(CurrentFieldAdditionError::Source)
    ));
}

#[test]
fn populated_short_text_add_preserves_records_and_independent_full_images() {
    use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text;
    for (units, compressed) in [(255, true), (1, false), (37, true)] {
        let [before, _] = populated_text::pair("Label", &[Some(42), None, Some(-7)]);
        let expected = populated_text::append_text_declaration(&before, "Label", units, compressed);
        let facts = image(&before)
            .inspect_current_short_text_field_addition_source("Table1", &mut budget(), &|| false)
            .unwrap();
        let candidate = image(&before)
            .prepare_current_short_text_field_addition(
                "Table1",
                "Label",
                units as usize,
                compressed,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        equal_image(candidate.before_bytes(), &before);
        equal_image(candidate.after_bytes(), &expected);
        assert_eq!(candidate.native_number(), 2);
        assert_eq!(candidate.native_number(), facts.native_number());
        assert_eq!(candidate.max_utf16_units(), units as usize);
        assert_eq!(candidate.unicode_compression(), compressed);
        assert_eq!(&expected[8 * PAGE..10 * PAGE], &before[8 * PAGE..10 * PAGE]);
        let rows = crate::read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(42), Value::Null],
                vec![Value::Int32(2), Value::Null, Value::Null],
                vec![Value::Int32(3), Value::Int32(-7), Value::Null]
            ]
        );
    }
}

#[test]
fn current_short_text_invalid_declarations_and_cancellation_do_not_produce_candidates() {
    let before = populated::pair("Unused", &[Some(42)])[0].to_vec();
    for units in [0, 256, usize::MAX] {
        assert!(
            image(&before)
                .prepare_current_short_text_field_addition(
                    "Table1",
                    "Label",
                    units,
                    true,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    assert!(matches!(
        image(&before).prepare_current_short_text_field_addition(
            "Table1",
            "Label",
            255,
            true,
            &mut budget(),
            &|| true
        ),
        Err(CurrentFieldAdditionError::Inspect(InspectError::Cancelled))
    ));
}
