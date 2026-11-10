//! Independent retained sources exercise source facts, alias rejection and paid work.
use super::Inventory;
use crate::{EditError, NativeImage};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target,
    },
    canonical_retained_add_table::{self as appended, Target as Added},
    canonical_table_drop::current::{self as raw, History},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 1_000_000, 8_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(874),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn live_retired_and_repeated_appends_keep_native_roles_independent_of_page_formulas() {
    for (case, target) in [
        (Case::NumberPresent, Target::Number),
        (Case::MixedBinary, Target::ShortText),
    ] {
        let source = raw::source(case, target, true, History::Inserted);
        let after = raw::after(&source);
        for (original, live) in [(&source.bytes, 9), (&after, 8)] {
            for declaration in appended::TARGETS {
                let image = capture(original);
                let inventory = Inventory::capture(&image, &mut budget(), &|| false).unwrap();
                inventory.require_source(&image).unwrap();
                assert_eq!(inventory.before_pages() as usize, original.len() / 4096);
                assert_eq!(inventory.catalog_definition(), 2);
                assert_eq!(inventory.catalog_pages(), [4]);
                assert_eq!(inventory.map_pages(), [3]);
                assert_eq!(inventory.tables_parent(), 0x0f00_0001);
                assert_eq!(inventory.catalog_live_rows(), live);
                let next = appended::append(original, "NewTable", declaration);
                let next = appended::append(&next, "AnotherTable", Added::GeneratedNumber);
                let image = capture(&next);
                let inventory = Inventory::capture(&image, &mut budget(), &|| false).unwrap();
                assert_eq!(inventory.before_pages() as usize, next.len() / 4096);
                assert!(inventory.before_pages() > 11);
            }
        }
    }
}

#[test]
fn another_equal_image_and_explicit_work_controls_cannot_reuse_inventory_authority() {
    let source = raw::source(Case::MixedBinary, Target::ShortText, true, History::Deleted);
    let image = capture(&source.bytes);
    let other = capture(&source.bytes);
    let inventory = Inventory::capture(&image, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        inventory.require_source(&other),
        Err(EditError::Preservation)
    ));
    assert!(Inventory::capture(&image, &mut budget(), &|| true).is_err());
    assert!(Inventory::capture(&image, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err());
}

#[test]
fn retired_forwarding_and_index_references_must_remain_inside_real_owned_storage() {
    let source = raw::source(Case::TextPresent, Target::ShortText, true, History::Initial);
    let after = raw::after(&source);
    let image = capture(&after);
    Inventory::capture(&image, &mut budget(), &|| false).unwrap();
    let mut bad_forward = after.clone();
    let at = 8 * 4096 + 14;
    let offset =
        usize::from(u16::from_le_bytes(bad_forward[at..at + 2].try_into().unwrap()) & 0x1fff);
    let end = 4096;
    assert_eq!(
        u16::from_le_bytes(source.bytes[at..at + 2].try_into().unwrap()) & 0xc000,
        0x4000
    );
    assert_eq!(end - offset, 11);
    bad_forward[8 * 4096 + offset..8 * 4096 + offset + 4]
        .copy_from_slice(&((11_u32 << 8) | 1).to_le_bytes());
    assert_eq!(
        &bad_forward[8 * 4096 + offset + 4..8 * 4096 + end],
        &after[8 * 4096 + offset + 4..8 * 4096 + end]
    );
    let mut bad_index = after;
    bad_index[9 * 4096 + 16..9 * 4096 + 20].copy_from_slice(&11_u32.to_le_bytes());
    for bytes in [bad_forward, bad_index] {
        let image = capture(&bytes);
        assert!(Inventory::capture(&image, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn map_aliases_and_wrong_existing_data_owner_refuse_before_any_append() {
    let source = raw::source(Case::NumberPresent, Target::Number, true, History::Fields);
    let mut aliased = source.bytes.clone();
    aliased[7 * 4096 + 55..7 * 4096 + 59].copy_from_slice(&(3_u32 << 8).to_le_bytes());
    let mut owner = source.bytes;
    owner[8 * 4096 + 4..8 * 4096 + 8].copy_from_slice(&2_u32.to_le_bytes());
    for bytes in [aliased, owner] {
        let image = capture(&bytes);
        assert!(Inventory::capture(&image, &mut budget(), &|| false).is_err());
    }
}
