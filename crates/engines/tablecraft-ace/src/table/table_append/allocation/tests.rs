//! Independent global-map oracles distinguish representation capacity from quotas.
use super::AllocationError;
use crate::{NativeImage, table::table_append::Inventory};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target,
    },
    canonical_retained_add_table::{self as raw, Target as Added},
    canonical_table_drop::current::{self, History},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 256 * 1024 * 1024)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(899),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn original() -> Vec<u8> {
    let source = current::source(Case::NumberPresent, Target::Number, true, History::Inserted);
    current::after(&source)
}

#[test]
fn sole_inline_global_row_grows_past_page_sixty_three_without_rewriting_old_bits() {
    let mut before = original();
    for number in 0..17 {
        before = raw::append(&before, &format!("Table{number}"), Added::GeneratedNumber);
    }
    assert_eq!(before.len(), 62 * 4096);
    let source = image(&before);
    let inventory = Inventory::capture(&source, &mut budget(), &|| false).unwrap();
    let expected = raw::append(&before, "More", Added::GeneratedNumber);
    assert_eq!(expected.len(), 65 * 4096);
    let allocation = inventory
        .physical()
        .plan_allocation(65, &mut budget(), &|| false)
        .unwrap();
    let mut candidate = expected.clone();
    candidate[4096..8192].copy_from_slice(&before[4096..8192]);
    allocation
        .apply(&mut candidate, &mut budget(), &|| false)
        .unwrap();
    allocation
        .verify_after(&candidate, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate, expected);
    let start = usize::from(u16::from_le_bytes(
        candidate[4096 + 14..4096 + 16].try_into().unwrap(),
    ));
    assert_eq!(4096 - start, 14);
    for (within, (old, new)) in before[4096..8192]
        .iter()
        .zip(&candidate[4096..8192])
        .enumerate()
    {
        if old != new {
            let offset = 4096 + within;
            assert!(
                allocation
                    .allowed_before_ranges()
                    .iter()
                    .any(|range| range.contains(&offset))
            );
        }
    }
    let mut forged = candidate.clone();
    forged[4096 + start + 5] ^= 1;
    assert!(
        allocation
            .verify_after(&forged, &mut budget(), &|| false)
            .is_err()
    );
    let mut wrong = candidate;
    wrong[4096..8192].copy_from_slice(&before[4096..8192]);
    wrong[4096 + 2] ^= 1;
    let unchanged = wrong.clone();
    assert!(
        allocation
            .apply(&mut wrong, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(wrong, unchanged);
}

#[test]
fn already_represented_future_free_bits_are_cleared_without_moving_the_global_row() {
    let mut before = original();
    let start = usize::from(u16::from_le_bytes(
        before[4096 + 14..4096 + 16].try_into().unwrap(),
    ));
    before[4096 + start + 5 + 1] |= 0x38;
    let source = image(&before);
    let inventory = Inventory::capture(&source, &mut budget(), &|| false).unwrap();
    let allocation = inventory
        .physical()
        .plan_allocation(14, &mut budget(), &|| false)
        .unwrap();
    let mut candidate = before.clone();
    candidate.resize(14 * 4096, 0);
    allocation
        .apply(&mut candidate, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        &candidate[4096 + 14..4096 + 16],
        &before[4096 + 14..4096 + 16]
    );
    assert_eq!(candidate[4096 + start + 5 + 1] & 0x38, 0);
    for (offset, (old, new)) in before.iter().zip(&candidate).enumerate() {
        if old != new {
            assert!(
                allocation
                    .allowed_before_ranges()
                    .iter()
                    .any(|range| range.contains(&offset))
            );
        }
    }
    allocation
        .verify_after(&candidate, &mut budget(), &|| false)
        .unwrap();
}

#[test]
fn bitmap_carrier_exhaustion_stays_distinct_from_source_and_explicit_work_failures() {
    let before = original();
    let source = image(&before);
    let inventory = Inventory::capture(&source, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        inventory
            .physical()
            .plan_allocation(524_288, &mut budget(), &|| false),
        Err(AllocationError::Capacity)
    ));
    assert!(matches!(
        inventory
            .physical()
            .plan_allocation(10, &mut budget(), &|| false),
        Err(AllocationError::Source)
    ));
    assert!(matches!(
        inventory
            .physical()
            .plan_allocation(14, &mut budget(), &|| true),
        Err(AllocationError::Inspect(_))
    ));
    assert!(matches!(
        inventory
            .physical()
            .plan_allocation(14, &mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(AllocationError::Inspect(_))
    ));
}
