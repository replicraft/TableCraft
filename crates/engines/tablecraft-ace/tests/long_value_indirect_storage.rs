use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "long_value_storage/model.rs"]
mod model;
#[allow(dead_code)]
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{
    InspectError, LongValueCheckStatus, TableLongValueCheck, check_table_long_values, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

fn check(bytes: &[u8]) -> Result<TableLongValueCheck, InspectError> {
    check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}

fn source(code: u8, owned_indirect: bool, hint_indirect: bool) -> Vec<u8> {
    let mut bytes = model::database(code);
    bytes.resize(22 * 4096, 0);
    let owned = if owned_indirect {
        vec![1, 21, 0, 0, 0]
    } else {
        model::map(&[14, 15, 16, 20])
    };
    let hint = if hint_indirect {
        vec![1, 21, 0, 0, 0]
    } else {
        model::map(&[20])
    };
    let maps = model::page(
        0,
        &[
            (0, model::map(&[12])),
            (0, owned),
            (0, hint),
            (0, model::map(&[17, 18, 19])),
            (0, model::map(&[19])),
        ],
    );
    bytes[11 * 4096..12 * 4096].copy_from_slice(&maps);
    bytes[21 * 4096..21 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
    for number in [14, 15, 16, 20] {
        bytes[21 * 4096 + 4 + number / 8] |= 1 << (number % 8);
    }

    // The literal bitmap is allocated. Future global bits remain untouched.
    let global_start = u16::from_le_bytes(bytes[4096 + 14..4096 + 16].try_into().unwrap());
    bytes[4096 + usize::from(global_start) + 5 + 21 / 8] &= !(1 << (21 % 8));
    bytes
}

#[test]
fn mixed_owned_indirect_and_inline_hint_checks_every_value_and_opaque_orphan() {
    for code in [5, 6] {
        let bytes = source(code, true, false);
        let original = bytes.clone();
        let rows = read_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows().len(), 5);

        let report = check(&bytes).unwrap();
        assert_eq!(report.rows(), 5);
        for column in report.columns() {
            assert_eq!(column.status(), LongValueCheckStatus::Checked);
            assert_eq!(column.values().nulls(), 1);
            assert_eq!(column.values().inline(), 2);
            assert_eq!(column.values().single(), 1);
            assert_eq!(column.values().chained(), 1);
            assert_eq!(column.distinct_chunks(), 3);
        }
        assert_eq!(report.columns()[0].owned_pages(), 4);
        assert_eq!(report.columns()[0].candidate_pages(), 1);
        assert_eq!(report.columns()[0].owned_rows(), 4);
        assert_eq!(report.columns()[0].orphan_rows(), 1);
        assert_eq!(bytes, original);
    }
}

#[test]
fn candidate_indirect_shapes_remain_visible_without_invented_checked_counts() {
    for code in [5, 6] {
        for owned_indirect in [false, true] {
            let bytes = source(code, owned_indirect, true);
            let report = check(&bytes).unwrap();
            assert_eq!(
                report.columns()[0].status(),
                LongValueCheckStatus::IndirectMaps
            );
            assert_eq!(report.columns()[0].owned_pages(), 0);
            assert_eq!(report.columns()[0].distinct_chunks(), 0);
            assert_eq!(report.columns()[1].status(), LongValueCheckStatus::Checked);
        }
    }
}

#[test]
fn mixed_map_checks_reject_free_bitmap_bad_signature_and_missing_live_membership() {
    for code in [5, 6] {
        let original = source(code, true, false);
        assert!(check(&original).is_ok());

        let mut free_bitmap = original.clone();
        let global_start =
            u16::from_le_bytes(free_bitmap[4096 + 14..4096 + 16].try_into().unwrap());
        free_bitmap[4096 + usize::from(global_start) + 5 + 21 / 8] |= 1 << (21 % 8);
        assert_eq!(check(&free_bitmap).err(), Some(InspectError::Malformed));

        let mut bad_signature = original.clone();
        bad_signature[21 * 4096] = 0;
        assert_eq!(check(&bad_signature).err(), Some(InspectError::Malformed));

        let mut missing_chunk = original.clone();
        missing_chunk[21 * 4096 + 4 + 16 / 8] &= !(1 << (16 % 8));
        assert_eq!(check(&missing_chunk).err(), Some(InspectError::Malformed));

        let mut missing_pointer = original.clone();
        let owned_slot = 11 * 4096 + 16;
        let owned_start = u16::from_le_bytes(
            missing_pointer[owned_slot..owned_slot + 2]
                .try_into()
                .unwrap(),
        );
        missing_pointer
            [11 * 4096 + usize::from(owned_start) + 1..11 * 4096 + usize::from(owned_start) + 5]
            .fill(0);
        assert_eq!(check(&missing_pointer).err(), Some(InspectError::Malformed));
    }
}
