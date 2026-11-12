//! Selection is metadata, never an excuse to skip the actual native proof.
use crate::{NativeImage, SecondaryIndexCreate};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as raw, Population, history,
};
fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(28621),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn count_free_and_unicode_original_routes_remain_original_while_history_selects_fresh() {
    let case = history::source();
    assert!(
        image(&case.before)
            .select_native_fresh_long_index_creation_source(case.table, &mut budget(), &|| false)
            .unwrap()
    );
    let contiguous = raw::protected_companion(false);
    assert!(
        !image(&contiguous.expected)
            .select_native_fresh_long_index_creation_source(
                contiguous.table,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    let plain = raw::source(Population::NullableDuplicate, false);
    let created = image(&plain.before)
        .prepare_retained_long_index_creation(
            plain.table,
            &SecondaryIndexCreate::new("ſcoreLookup", plain.column, false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(
        !image(created.candidate_bytes())
            .select_native_fresh_long_index_creation_source(plain.table, &mut budget(), &|| false)
            .unwrap()
    );
}

#[test]
fn metadata_selection_neither_reads_target_records_nor_certifies_a_corrupt_original() {
    let case = history::source();
    let mut corrupt = case.before.to_vec();
    corrupt[(case.target_definition as usize + 1) * 4096] = 255;
    let image = image(&corrupt);
    assert!(
        image
            .select_native_fresh_long_index_creation_source(case.table, &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        image
            .prepare_retained_fresh_long_index_creation(
                case.table,
                &SecondaryIndexCreate::new(case.index_name, case.column, false),
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
}

#[test]
fn selector_errors_do_not_become_an_original_mode_answer() {
    let case = history::source();
    let image = image(&case.before);
    assert!(
        image
            .select_native_fresh_long_index_creation_source("Missing", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image
            .select_native_fresh_long_index_creation_source(case.table, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image
            .select_native_fresh_long_index_creation_source(
                case.table,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}
