#[allow(dead_code)]
#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "stored_index_projection/fixture.rs"]
mod indexed;
use std::{cell::Cell, io::Cursor, ops::Bound};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, InspectError, Int32IndexSelection,
    read_stored_table_index_in_table_order, read_stored_table_index_with_references,
    read_stored_table_index_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

fn range(first: i32, last: i32) -> IndexProjectionSelection<'static> {
    IndexProjectionSelection::Int32(Int32IndexSelection::Range {
        lower: Bound::Included(first),
        upper: Bound::Included(last),
    })
}

fn request(selection: IndexProjectionSelection<'static>, cap: u32) -> IndexReadRequest<'static> {
    IndexReadRequest {
        columns: Some(&["Score"]),
        selection,
        maximum_rows: cap,
    }
}

#[test]
fn selected_cached_rows_use_returned_caps_and_keep_checked_logical_references() {
    for profile in [5, 6] {
        let mut bytes = indexed::database(&[Some(77), None, Some(-4)], false, false);
        bytes[20] = profile;
        assert!(matches!(
            read_stored_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "IdOrder",
                request(IndexProjectionSelection::All, 1),
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::StructureLimit)
        ));
        let selected = read_stored_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(range(2, 2), 1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(selected.snapshot().rows(), &[vec![Value::Null]]);
        assert_eq!(selected.snapshot().schema().declared_rows(), 3);
        assert_eq!(selected.row_references()[0].page_number(), 12);
        assert_eq!(selected.row_references()[0].slot_number(), 1);
        let absent = read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(range(4, 5), 1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(absent.rows().is_empty());
        let mut invalid = bytes.clone();
        invalid[10 * indexed::PAGE + 75 + 25 + 23] = 38;
        assert!(matches!(
            read_stored_table_index_with_request(
                &mut Cursor::new(&invalid),
                CatalogScope::new(1),
                "Items",
                "IdOrder",
                request(range(4, 5), 1),
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn table_order_is_logical_order_even_with_descending_keys_and_forwarded_values() {
    let bytes = indexed::forwarded(indexed::database(&[Some(77), None, Some(-4)], true, true));
    let selected = read_stored_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdTwin",
        request(range(1, 3), 3),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        selected.rows(),
        &[
            vec![Value::Int32(-4)],
            vec![Value::Null],
            vec![Value::Int32(77)]
        ]
    );
    let ordered = read_stored_table_index_in_table_order(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdTwin",
        request(range(1, 3), 3),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        ordered.rows(),
        &[
            vec![Value::Int32(77)],
            vec![Value::Null],
            vec![Value::Int32(-4)]
        ]
    );
    let referenced = read_stored_table_index_with_references(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(range(1, 1), 1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(referenced.row_references()[0].page_number(), 12);
    assert_eq!(referenced.row_references()[0].slot_number(), 0);
}

fn leaf(entries: &[(i32, u32)], previous: u32, next: u32) -> Vec<u8> {
    let mut bytes = vec![0; 4096];
    bytes[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    let mut active = 0;
    for &(key, row) in entries {
        let mut entry = vec![0x7f];
        entry.extend(((key as u32) ^ 0x80000000).to_be_bytes());
        entry.extend(row.to_be_bytes());
        bytes[480 + active..480 + active + entry.len()].copy_from_slice(&entry);
        active += entry.len();
        bytes[27 + active / 8] |= 1 << (active % 8);
    }
    support::put16(&mut bytes, 2, (4096 - 480 - active) as u16);
    bytes
}

fn nodes() -> Vec<u8> {
    let mut bytes = indexed::database(&[Some(77), Some(88), Some(99)], false, false);
    bytes.resize(17 * indexed::PAGE, 0);
    bytes[14 * indexed::PAGE..15 * indexed::PAGE].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, 1])],
    ));
    let mut root = leaf(&[], 0, 0);
    root[0] = 3;
    root[26] = 1;
    support::put32(&mut root, 20, 16);
    let mut bound = vec![0x7f];
    bound.extend((2_u32 ^ 0x80000000).to_be_bytes());
    bound.extend(((12_u32 << 8) | 1).to_be_bytes());
    bound.extend(15_u32.to_be_bytes());
    root[480..493].copy_from_slice(&bound);
    root[28] = 0x20;
    support::put16(&mut root, 2, 3603);
    bytes[13 * indexed::PAGE..14 * indexed::PAGE].copy_from_slice(&root);
    bytes[15 * indexed::PAGE..16 * indexed::PAGE].copy_from_slice(&leaf(
        &[(1, 12 << 8), (2, (12 << 8) | 1)],
        0,
        16,
    ));
    bytes[16 * indexed::PAGE..17 * indexed::PAGE].copy_from_slice(&leaf(
        &[(3, (12 << 8) | 2)],
        15,
        0,
    ));
    bytes
}

#[test]
fn skipped_cache_and_subtree_damage_stays_outside_partial_read_scope() {
    let original = nodes();
    let baseline = read_stored_table_index_with_request(
        &mut Cursor::new(&original),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(IndexProjectionSelection::All, 3),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(baseline.rows().len(), 3);
    for damage in [0, 1] {
        let mut bytes = original.clone();
        if damage == 0 {
            bytes[16 * indexed::PAGE] = 0;
        } else {
            let start = indexed::row_start(&bytes, 2);
            bytes[start + 6] = 1;
        }
        let (last, expected) = if damage == 0 {
            // The right subtree starts after (key2, ROW1), not after every key2.
            // An inclusive key2 upper bound must still visit it for duplicate keys.
            assert!(matches!(
                read_stored_table_index_with_request(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "IdOrder",
                    request(range(1, 2), 2),
                    &mut budget(),
                    &|| false,
                ),
                Err(InspectError::Malformed)
            ));
            (1, vec![vec![Value::Int32(77)]])
        } else {
            // This subtree remains structurally valid; its key3 payload is unselected.
            (2, vec![vec![Value::Int32(77)], vec![Value::Int32(88)]])
        };
        let selected = read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(range(1, last), last as u32),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(selected.rows(), expected);
        assert!(
            read_stored_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "IdOrder",
                request(range(3, 3), 1),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert!(
            read_stored_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "IdOrder",
                request(IndexProjectionSelection::All, 3),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn selected_caps_and_terminal_cancellation_never_publish_partial_results() {
    let bytes = nodes();
    assert!(matches!(
        read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(range(1, 2), 1),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::StructureLimit)
    ));
    let calls = Cell::new(0);
    read_stored_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(range(1, 2), 2),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let terminal = calls.get() - 1;
    calls.set(0);
    assert!(matches!(
        read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(range(1, 2), 2),
            &mut budget(),
            &|| {
                let call = calls.get();
                calls.set(call + 1);
                call == terminal
            },
        ),
        Err(InspectError::Cancelled)
    ));
}
