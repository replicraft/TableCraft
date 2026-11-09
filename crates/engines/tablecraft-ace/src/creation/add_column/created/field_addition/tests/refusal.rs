use super::*;
use crate::{
    CreateColumn, CreatedDefaultZeroColumnRequired, FirstTableDefinition, create_ace16_database,
};
use tablecraft_schema::ScalarType;

const PAGE_BYTES: usize = 4096;
const USER_DEFINITION: usize = 7;
const FIRST_DESCRIPTOR: usize = 75;
const DESCRIPTOR_BYTES: usize = 25;
const EXTENDED_FLAGS: usize = 16;

#[test]
fn compression_opt_in_preserves_old_constructor_and_frozen_definition_languages() {
    let ordinary = CreateColumn::short_text("Label", 255).unwrap();
    assert!(!ordinary.unicode_compression());
    assert!(
        ordinary
            .with_unicode_compression(true)
            .unwrap()
            .unicode_compression()
    );
    assert!(
        !ordinary
            .with_unicode_compression(false)
            .unwrap()
            .unicode_compression()
    );
    assert!(
        CreateColumn::new("Score", ScalarType::Int32)
            .with_unicode_compression(true)
            .is_err()
    );
    let primary = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let compressed = ordinary.with_unicode_compression(true).unwrap();
    assert!(
        FirstTableDefinition::capture("Table1", &[primary, compressed], &mut budget(), &|| false)
            .is_err()
    );
    let id = CreateColumn::new("ID", ScalarType::Int32);
    assert!(
        FirstTableDefinition::capture_manual("Table1", &[id, compressed], &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn compressed_mixed_source_never_widens_frozen_add_column_profiles() {
    let before = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    assert!(
        image(&before)
            .prepare_add_int32_column_candidate("Table1", "Other", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_created_add_int32_column_candidate("Table1", "Other", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Other",
                CreatedDefaultZeroColumnRequired::StoredFalse,
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                "Other",
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn invalid_input_and_resource_refusals_publish_no_candidate_or_partial_bytes() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let original = before.clone();
    for (name, target) in [
        ("ID", text(255, true)),
        ("Bad.Name", text(255, true)),
        ("Label", text(0, true)),
        ("Label", text(256, true)),
    ] {
        assert!(
            image(&before)
                .prepare_created_add_field_candidate("Table1", name, target, &mut budget(), &|| {
                    false
                })
                .is_err()
        );
    }
    assert!(
        image(&before)
            .prepare_created_add_field_candidate(
                "Table1",
                "Label",
                text(255, true),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_created_add_field_candidate(
                "Table1",
                "Label",
                text(255, true),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert_eq!(before, original);
}

#[test]
fn unknown_compression_flags_history_and_protected_complement_refuse() {
    let before = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    for (offset, mask) in [
        (
            USER_DEFINITION * PAGE_BYTES + FIRST_DESCRIPTOR + DESCRIPTOR_BYTES + EXTENDED_FLAGS,
            2,
        ),
        (10 * PAGE_BYTES + PAGE_BYTES - 1, 1),
        (PAGE_BYTES - 1, 1),
        (9 * PAGE_BYTES + PAGE_BYTES - 1, 1),
    ] {
        let mut changed = before.clone();
        changed[offset] ^= mask;
        if let Ok(source) = NativeImage::capture(
            &mut Cursor::new(&changed),
            CatalogScope::new(901),
            &mut budget(),
            &|| false,
        ) {
            assert!(
                source
                    .prepare_created_add_field_candidate(
                        "Table1",
                        "OtherLabel",
                        text(255, true),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
    // A valid canonical source with a different policy remains outside the
    // new history language; creator compatibility is not inferred from shape.
    let id = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let required_text = CreateColumn::short_text("Label", 80)
        .unwrap()
        .with_required(true)
        .with_allow_zero_length(true)
        .unwrap();
    let unrelated =
        create_ace16_database("Table1", &[id, required_text], &[], &mut budget(), &|| {
            false
        })
        .unwrap();
    assert!(
        image(unrelated.as_bytes())
            .prepare_created_add_field_candidate(
                "Table1",
                "OtherLabel",
                text(255, true),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}
