//! Policy selection remains metadata-only and grants no mutation capability.
use crate::NativeImage;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target as Field,
    },
    canonical_creation_primary,
    canonical_retained_add_table::{self as added, Target},
    canonical_table_drop::current as dropped,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn reader_and_image_metadata_select_the_same_family_without_source_fallback() {
    let current = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Fields,
    );
    let retired = dropped::after(&current);
    let retained = added::desktop::append(&retired, "Fresh", Target::GeneratedText);
    for (bytes, name, expected) in [
        (
            canonical_creation_primary::source(canonical_creation_primary::Case::Empty).bytes,
            "Items",
            false,
        ),
        (current.bytes, "Table1", false),
        (retained, "Fresh", true),
    ] {
        let scope = CatalogScope::new(3111);
        assert_eq!(
            NativeImage::select_retained_table_source(
                &mut Cursor::new(&bytes),
                scope,
                name,
                &mut budget(),
                &|| false
            )
            .unwrap(),
            expected
        );
        let image =
            NativeImage::capture_retained(&mut Cursor::new(&bytes), scope, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(
            image
                .has_retained_table_source(name, &mut budget(), &|| false)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn large_original_metadata_selection_spends_no_whole_image_retention_budget() {
    let current = dropped::source(
        Case::TextPresent,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let retired = dropped::after(&current);
    let mut bytes = added::desktop::append(&retired, "Fresh", Target::GeneratedText);
    bytes.resize(64 * 1024 * 1024 + 4096, 0);
    let scope = CatalogScope::new(3112);
    // The caller owns deliberately unqualified trailing pages. This small
    // metadata budget can select capture policy, but proves no write source.
    assert!(
        NativeImage::select_retained_table_source(
            &mut Cursor::new(&bytes),
            scope,
            "Fresh",
            &mut budget(),
            &|| false
        )
        .unwrap()
    );
    assert!(
        NativeImage::select_retained_table_source(
            &mut Cursor::new(&bytes),
            scope,
            "Fresh",
            &mut budget(),
            &|| true
        )
        .is_err()
    );
    let mut empty = ResourceBudget::new(0, 0, 0);
    assert!(
        NativeImage::select_retained_table_source(
            &mut Cursor::new(&bytes),
            scope,
            "Fresh",
            &mut empty,
            &|| false
        )
        .is_err()
    );
}
