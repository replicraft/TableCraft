//! Complete SDK-pinned AFTERs and independently authored source refusals.
use super::*;
use crate::{CreatedDefaultZeroColumnRequired, NativeImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::{Source, repeated as raw},
    support::bytes::hex,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(897),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn qualify(case: Source, table: &str, column: &str, source_hash: &str, after_hash: &str) {
    let before = raw::source(case);
    let expected = raw::append(&before, column);
    assert_eq!(hash(&before), source_hash);
    assert_eq!(hash(&expected), after_hash);
    image(&before)
        .check_repeated_default_zero_add_column_source(table, &mut budget(), &|| false)
        .unwrap();
    let candidate = image(&before)
        .prepare_repeated_default_zero_int32_column_candidate(table, column, &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.after_bytes(), expected);
    assert_eq!(candidate.table(), table);
    assert_eq!(candidate.column(), column);
    assert_eq!(
        candidate.before_bytes().len(),
        candidate.after_bytes().len()
    );
    assert!(
        image(&before)
            .prepare_created_add_default_zero_int32_column_candidate(
                table,
                column,
                CreatedDefaultZeroColumnRequired::StoredFalse,
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
}

#[test]
fn id_only_second_number_matches_complete_independent_sdk_after() {
    qualify(
        Source::IdOnly,
        "Table1",
        "Score2",
        "a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72",
        "8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9",
    );
}

#[test]
fn required_first_second_number_matches_complete_independent_sdk_after() {
    qualify(
        Source::GuiRequiredZero,
        "Table1",
        "Extra2",
        "b314c02b5f67c8deebb564f7e4deae5cd5a5e28e1f67f3122b1cb8d14507c3a4",
        "d3313771325a70522d7c52e7f2094fd1ef1e17e349651a25c029c88fe858cba1",
    );
}

#[test]
fn absent_old_required_second_number_matches_complete_independent_sdk_after() {
    qualify(
        Source::DefaultOnly,
        "Items",
        "Extra2",
        "05dcae27ffd63cba552e47740c60649d46bb265730b6b202a384e69ebb249072",
        "d8e3fdfc029324c38784bb93be352ffef9df0a383ecd4f8952596df9ad78b647",
    );
}

#[test]
fn next_history_stage_is_reconstructed_with_all_old_property_rows_preserved() {
    let first = raw::source(Source::IdOnly);
    let before = raw::append(&first, "Score2");
    let expected = raw::append(&before, "Third Number");
    let actual = image(&before)
        .prepare_repeated_default_zero_int32_column_candidate(
            "Table1",
            "Third Number",
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(actual.after_bytes(), expected);
}

#[test]
fn stale_history_descriptor_maps_and_unknown_complement_never_gain_a_candidate() {
    let before = raw::source(Source::IdOnly);
    let catalog = &before[4 * 4096..5 * 4096];
    let root_start = u16::from_le_bytes(catalog[28..30].try_into().unwrap()) as usize & 0x1fff;
    let target_start = u16::from_le_bytes(catalog[30..32].try_into().unwrap()) as usize & 0x1fff;
    let row = &catalog[target_start..root_start];
    let at = row.len() - 5 - 18;
    let property = u16::from_le_bytes(row[at..at + 2].try_into().unwrap()) as usize;
    let descriptor_row = 4 * 4096 + target_start + property + 4;
    for (offset, mask) in [
        (10 * 4096 + 4095, 1),
        (3 * 4096 + 3868, 4),
        (4096 - 1, 1),
        (descriptor_row, 1),
    ] {
        let mut changed = before.clone();
        changed[offset] ^= mask;
        if let Ok(source) = NativeImage::capture(
            &mut Cursor::new(&changed),
            CatalogScope::new(897),
            &mut budget(),
            &|| false,
        ) {
            assert!(
                source
                    .prepare_repeated_default_zero_int32_column_candidate(
                        "Table1",
                        "Score2",
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                "Score",
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                "Score2",
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                "Score2",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn full_existing_property_page_refuses_without_allocation_or_partial_candidate() {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=12 {
        before = raw::append(&before, &format!("N{number}"));
    }
    let untouched = before.clone();
    image(&before)
        .check_repeated_default_zero_add_column_source("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                &"W".repeat(64),
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    assert_eq!(before, untouched);
}
