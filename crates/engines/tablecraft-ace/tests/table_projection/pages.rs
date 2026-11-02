//! Bounded explicit projection shares paging geometry, controls and EOF rules.
use super::*;
use std::cell::Cell;
use tablecraft_ace::{TablePageRequest, read_table_page, read_table_projection_page};
use tablecraft_core::limits::Resource;
use tablecraft_testkit::support::ace::page_reads::PageReads;

#[test]
fn projected_pages_keep_full_schema_native_column_order_and_exact_live_offsets() {
    let bytes = fixture::database();
    for offset in [0, 1, 2, 3, u32::MAX] {
        for names in [&["score", "ID"][..], &["Score"][..], &[][..]] {
            let page = read_table_projection_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                names,
                TablePageRequest::new(offset, 1).unwrap(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(page.schema().columns().len(), 2);
            assert_eq!(page.columns().len(), names.len());
            assert_eq!(page.request().offset(), offset);
            if offset < 3 {
                let id = offset as i32 + 1;
                let expected = match names.len() {
                    0 => vec![],
                    1 => vec![Value::Int32(id * 10)],
                    _ => vec![Value::Int32(id), Value::Int32(id * 10)],
                };
                assert_eq!(page.rows(), [expected]);
                assert!(!page.reached_end());
            } else {
                assert!(page.rows().is_empty());
                assert!(page.reached_end());
            }
            if names.len() == 2 {
                assert_eq!(
                    page.columns()
                        .iter()
                        .map(|column| column.name())
                        .collect::<Vec<_>>(),
                    ["Id", "Score"]
                );
            }
        }
    }
    let last_full = read_table_projection_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        TablePageRequest::new(0, 3).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(last_full.rows().len(), 3);
    assert!(!last_full.reached_end());
}

#[test]
fn unselected_unknown_values_are_omitted_but_names_and_fetched_framing_still_qualify() {
    let mut bytes = fixture::database();
    bytes[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = UNKNOWN_TYPE;
    assert!(matches!(
        read_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported)
    ));
    let page = read_table_projection_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        TablePageRequest::new(0, 1).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(page.rows(), [vec![Value::Int32(1)]]);
    for names in [&["Score"][..], &["Id", "ID"][..], &["missing"][..]] {
        let mut input = PageReads::new(bytes.clone());
        assert!(
            read_table_projection_page(
                &mut input,
                CatalogScope::new(1),
                "Items",
                names,
                TablePageRequest::new(u32::MAX, 1).unwrap(),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(input.count(12), 0);
    }
    let start = usize::from(
        u16::from_le_bytes(
            bytes[12 * PAGE_BYTES + 14..12 * PAGE_BYTES + 16]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    support::put16(&mut bytes, 12 * PAGE_BYTES + start, u16::MAX);
    for names in [&["Id"][..], &[][..]] {
        assert!(
            read_table_projection_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                names,
                TablePageRequest::new(0, 1).unwrap(),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
    // Existing offset paging checks skipped root bounds, not skipped payloads.
    let page = read_table_projection_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        TablePageRequest::new(1, 1).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(page.rows(), [vec![Value::Int32(2)]]);
}

#[test]
fn selected_pages_validate_declared_counts_only_at_actual_eof_and_preserve_all_pools() {
    let mut bytes = fixture::database();
    support::put32(&mut bytes, OWNER_PAGE * PAGE_BYTES + 16, 4);
    let first = read_table_projection_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        TablePageRequest::new(0, 1).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(!first.reached_end());
    assert!(matches!(
        read_table_projection_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &["Id"],
            TablePageRequest::new(0, 4).unwrap(),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed)
    ));

    let bytes = fixture::database();
    let mut measured = budget();
    let before = [
        measured.remaining(Resource::Bytes),
        measured.remaining(Resource::Rows),
        measured.remaining(Resource::Steps),
    ];
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let page = read_table_projection_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        TablePageRequest::new(0, 4).unwrap(),
        &mut measured,
        &observe,
    )
    .unwrap();
    assert!(page.reached_end());
    let used = [
        before[0] - measured.remaining(Resource::Bytes),
        before[1] - measured.remaining(Resource::Rows),
        before[2] - measured.remaining(Resource::Steps),
    ];
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    assert_eq!(
        read_table_projection_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &["Id"],
            TablePageRequest::new(0, 4).unwrap(),
            &mut exact,
            &|| false,
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    for pool in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        assert_eq!(exact.remaining(pool), 0);
    }

    for (position, pool) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        assert!(used[position] > 0);
        let mut remaining = used;
        remaining[position] -= 1;
        assert!(matches!(read_table_projection_page(
            &mut Cursor::new(&bytes), CatalogScope::new(1), "Items", &["Id"],
            TablePageRequest::new(0,4).unwrap(), &mut ResourceBudget::new(remaining[0],remaining[1],remaining[2]), &|| false,
        ),Err(InspectError::Limit(error)) if error.resource==pool));
    }
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(matches!(
            read_table_projection_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &["Id"],
                TablePageRequest::new(0, 4).unwrap(),
                &mut budget(),
                &cancel,
            ),
            Err(InspectError::Cancelled)
        ));
    }
}
