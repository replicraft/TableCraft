#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "ace16_delete_trees/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{EditError, InspectError, NativeImage, RowDelete, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const ROOT: usize = 14;
const FIRST_LEAF: usize = 15;
const MAP_BYTES: usize = 69;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 20_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn database(bounded: bool) -> Vec<u8> {
    let (mut bytes, _) = fixture::database(6, 4, false, bounded);
    support::put32(&mut bytes, 13 * PAGE_BYTES + 4, 0);
    support::put32(&mut bytes, 11 * PAGE_BYTES + 4, 0);
    let mut global = vec![0; MAP_BYTES];
    for page in bytes.len() / PAGE_BYTES..512 {
        global[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
}

fn requests(bytes: &[u8], selected: &[usize]) -> Vec<RowDelete> {
    selected
        .iter()
        .map(|&row| RowDelete::new(row, fixture::raw_record(bytes, row)))
        .collect()
}

#[test]
fn retiring_a_first_leaf_repairs_chain_and_preserves_nonempty_root_height() {
    for bounded in [false, true] {
        let bytes = database(bounded);
        let candidate = capture(&bytes)
            .preview_ace16_deletes("Items", &requests(&bytes, &[0, 1]), &mut budget(), &|| {
                false
            })
            .unwrap();
        let after = candidate.candidate_bytes();
        assert_eq!(
            &after[FIRST_LEAF * PAGE_BYTES..FIRST_LEAF * PAGE_BYTES + 4],
            &[0; 4]
        );
        assert_eq!(
            &after[FIRST_LEAF * PAGE_BYTES + 4..(FIRST_LEAF + 1) * PAGE_BYTES],
            &bytes[FIRST_LEAF * PAGE_BYTES + 4..(FIRST_LEAF + 1) * PAGE_BYTES]
        );
        assert_eq!(after[ROOT * PAGE_BYTES], 3);
        assert_eq!(after[ROOT * PAGE_BYTES + 26], 1);
        assert_eq!(&after[16 * PAGE_BYTES + 12..16 * PAGE_BYTES + 16], &[0; 4]);
        assert_eq!(candidate.deleted_rows(), 2);
        check_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
fn delete_all_preserves_root_identity_and_aggregates_shared_global_map_bytes() {
    for bounded in [false, true] {
        let bytes = database(bounded);
        let selected = (0..8).collect::<Vec<_>>();
        let candidate = capture(&bytes)
            .preview_ace16_deletes(
                "Items",
                &requests(&bytes, &selected),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let after = candidate.candidate_bytes();
        assert_eq!(after[ROOT * PAGE_BYTES], 4);
        assert_eq!(after[ROOT * PAGE_BYTES + 26], 0);
        assert_eq!(
            &after[ROOT * PAGE_BYTES + 12..ROOT * PAGE_BYTES + 24],
            &[0; 12]
        );
        for page in 15..=17 {
            assert_eq!(&after[page * PAGE_BYTES..page * PAGE_BYTES + 4], &[0; 4]);
            assert_eq!(
                &after[page * PAGE_BYTES + 4..(page + 1) * PAGE_BYTES],
                &bytes[page * PAGE_BYTES + 4..(page + 1) * PAGE_BYTES]
            );
        }
        let start = u16::from_le_bytes(bytes[PAGE_BYTES + 14..PAGE_BYTES + 16].try_into().unwrap())
            as usize;
        let mut expected = bytes[PAGE_BYTES + start..2 * PAGE_BYTES].to_vec();
        for page in 15..=17 {
            expected[5 + page / 8] |= 1 << (page % 8);
        }
        assert_eq!(&after[PAGE_BYTES + start..2 * PAGE_BYTES], expected);
        assert_eq!(candidate.before_bytes(), bytes);
        assert_eq!(candidate.candidate_bytes().len(), bytes.len());
        assert_eq!(candidate.deleted_rows(), 8);
    }
}

#[test]
fn missing_global_representation_and_already_free_owned_pages_refuse_atomically() {
    let valid = database(true);
    let mut missing = valid.clone();
    missing[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, vec![0; 5])]));
    assert_eq!(
        capture(&missing)
            .preview_ace16_deletes(
                "Items",
                &requests(&missing, &[0, 1]),
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Inspect(InspectError::Unsupported))
    );
    let mut free = valid.clone();
    let start =
        u16::from_le_bytes(free[PAGE_BYTES + 14..PAGE_BYTES + 16].try_into().unwrap()) as usize;
    free[PAGE_BYTES + start + 5 + ROOT / 8] |= 1 << (ROOT % 8);
    assert_eq!(
        capture(&free)
            .preview_ace16_deletes("Items", &requests(&free, &[0, 1]), &mut budget(), &|| false)
            .err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    // Existing no-retirement behavior does not require allocator metadata.
    let (plain, _) = fixture::database(6, 4, false, true);
    assert!(
        capture(&plain)
            .preview_ace16_deletes("Items", &requests(&plain, &[1]), &mut budget(), &|| false)
            .is_ok()
    );
}

#[test]
fn globally_free_metadata_or_target_data_cannot_authorize_retirement() {
    for free_page in [0, 1, 10, 11, 12, 13] {
        let mut bytes = database(true);
        let start = u16::from_le_bytes(bytes[PAGE_BYTES + 14..PAGE_BYTES + 16].try_into().unwrap())
            as usize;
        // Full row/index checking does not inspect global allocator state.
        // These roles cover header, global container, table definition, owned
        // data-map container, actual tombstoned data, and index-map container.
        bytes[PAGE_BYTES + start + 5 + free_page / 8] |= 1 << (free_page % 8);
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            capture(&bytes)
                .preview_ace16_deletes("Items", &requests(&bytes, &[0, 1]), &mut budget(), &|| {
                    false
                })
                .is_err(),
            "globally free page {free_page}"
        );
    }
}
