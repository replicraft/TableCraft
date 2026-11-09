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
    let expected = raw::append(&before, "Third");
    assert_eq!(
        hash(&expected),
        "337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6"
    );
    let actual = image(&before)
        .prepare_repeated_default_zero_int32_column_candidate(
            "Table1",
            "Third",
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
fn full_existing_property_page_rolls_over_without_changing_old_page() {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=12 {
        before = raw::append(&before, &format!("N{number}"));
    }
    let untouched = before.clone();
    image(&before)
        .check_repeated_default_zero_add_column_source("Table1", &mut budget(), &|| false)
        .unwrap();
    let name = "W".repeat(64);
    let expected = raw::append(&before, &name);
    let actual = image(&before)
        .prepare_repeated_default_zero_int32_column_candidate(
            "Table1",
            &name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(actual.after_bytes(), expected);
    assert_eq!(
        &actual.after_bytes()[10 * 4096..11 * 4096],
        &before[10 * 4096..]
    );
    assert_eq!(before, untouched);
}

#[test]
fn sdk_n14_rollover_and_fitted_n15_match_complete_raw_images() {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=13 {
        before = raw::append(&before, &format!("N{number}"));
    }
    assert_eq!(
        hash(&before),
        "392e135acfc93ffe116c772fc9e728892a118a5792edd7a29b975d4e38b6c282"
    );
    let expected = raw::append(&before, "N14");
    assert_eq!(
        hash(&expected),
        "97fb669a90e6a733ddd09e47d548ca4743a833e68f9c2fe46631a8398c31c64d"
    );
    for name in ["N14", "N15"] {
        let expected = raw::append(&before, name);
        let untouched = before.clone();
        let candidate = image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.before_bytes(), untouched);
        assert_eq!(candidate.after_bytes(), expected);
        assert_eq!(
            &candidate.after_bytes()[10 * 4096..11 * 4096],
            &before[10 * 4096..11 * 4096]
        );
        before = expected;
    }
}

#[test]
fn wrong_eof_cell_or_foreign_ownership_cannot_gain_rollover_authority() {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=13 {
        before = raw::append(&before, &format!("N{number}"));
    }
    // Literal independent native map rows: global-free, table-owned and
    // LvProp-owned future page11 must all have their observed source polarity.
    for offset in [3 * 4096 + 4076, 3 * 4096 + 4089, 3 * 4096 + 3881] {
        let mut changed = before.clone();
        changed[offset] ^= 8;
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
                        "N14",
                        &mut budget(),
                        &|| false,
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn a_full_second_property_page_refuses_a_second_allocation() {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=9 {
        let name = format!("N{number}{}", "X".repeat(62));
        before = raw::append(&before, &name);
    }
    assert_eq!(before.len(), 12 * 4096);
    image(&before)
        .check_repeated_default_zero_add_column_source("Table1", &mut budget(), &|| false)
        .unwrap();
    let untouched = before.clone();
    let name = format!("N10{}", "X".repeat(61));
    assert!(
        image(&before)
            .prepare_repeated_default_zero_int32_column_candidate(
                "Table1",
                &name,
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    assert_eq!(before, untouched);
}
