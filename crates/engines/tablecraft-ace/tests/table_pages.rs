#[path = "table_pages/cursor.rs"]
mod cursor;
#[path = "table_pages/faults.rs"]
mod faults;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "table_pages/large_long_values.rs"]
mod large_long_values;
#[path = "table_pages/large_sources.rs"]
mod large_sources;
#[path = "large_maps/fixture.rs"]
mod map_fixture;
#[path = "table_pages/maps.rs"]
mod maps;
#[path = "table_pages/materialize.rs"]
mod materialize;
#[path = "support/page_reads.rs"]
mod page_reads;
use page_reads::PageReads;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use support::{data_page, put16, put32};
use tablecraft_ace::{InspectError, TablePageRequest, read_table, read_table_page};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE_BYTES: usize = 4096;
const TABLE_DEFINITION: usize = 10;
const USAGE_PAGE: usize = 11;
const FIRST_DATA_PAGE: usize = 12;
const RECORD_COUNT_OFFSET: usize = 16;
const ROWS_PER_PAGE: usize = 200;
const SCOPE: CatalogScope = CatalogScope::new(1);
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn row(id: i32) -> Vec<u8> {
    [
        2_u16.to_le_bytes().as_slice(),
        &id.to_le_bytes(),
        &(id * 10).to_le_bytes(),
        &[3],
    ]
    .concat()
}
fn database(count: usize) -> Vec<u8> {
    let pages = count.div_ceil(ROWS_PER_PAGE).max(1);
    let mut bytes = fixture::database();
    bytes.resize((FIRST_DATA_PAGE + pages) * PAGE_BYTES, 0);
    put32(
        &mut bytes,
        TABLE_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET,
        count as u32,
    );
    let mut map = vec![0; 5 + (FIRST_DATA_PAGE + pages).div_ceil(8)];
    for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + pages {
        map[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(1, &[(0, map)]));
    for index in 0..pages {
        let rows = (index * ROWS_PER_PAGE..((index + 1) * ROWS_PER_PAGE).min(count))
            .map(|i| (0, row(i as i32 + 1)))
            .collect::<Vec<_>>();
        bytes[(FIRST_DATA_PAGE + index) * PAGE_BYTES..(FIRST_DATA_PAGE + index + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(TABLE_DEFINITION as u32, &rows));
    }
    bytes
}
fn expected(offset: usize, count: usize) -> Vec<Vec<Value>> {
    (offset..offset + count)
        .map(|i| {
            vec![
                Value::Int32(i as i32 + 1),
                Value::Int32((i as i32 + 1) * 10),
            ]
        })
        .collect()
}
#[test]
fn pages_do_not_read_later_data_pages_or_decode_skipped_rows() {
    let bytes = database(600);
    let mut input = PageReads::new(bytes.clone());
    let mut page_budget = budget();
    let request = TablePageRequest::new(0, 2).unwrap();
    let page = read_table_page(
        &mut input,
        SCOPE,
        "Items",
        request,
        &mut page_budget,
        &|| false,
    )
    .unwrap();
    assert_eq!(page.rows(), expected(0, 2));
    assert_eq!(page.request(), request);
    assert_eq!(page.columns().len(), 2);
    assert!(!page.reached_end());
    assert_eq!(input.count(FIRST_DATA_PAGE as u64), 1);
    assert_eq!(input.count((FIRST_DATA_PAGE + 1) as u64), 0);
    let mut full_input = PageReads::new(bytes);
    let mut full_budget = budget();
    assert_eq!(
        read_table(&mut full_input, SCOPE, "Items", &mut full_budget, &|| false)
            .unwrap()
            .rows()
            .len(),
        600
    );
    assert_eq!(full_input.count((FIRST_DATA_PAGE + 2) as u64), 1);
    assert!(page_budget.remaining(Resource::Bytes) > full_budget.remaining(Resource::Bytes));
    assert!(page_budget.remaining(Resource::Rows) > full_budget.remaining(Resource::Rows));
}
#[test]
fn large_total_tables_and_offsets_are_not_limited_to_snapshot_row_count() {
    let bytes = database(18_000);
    assert!(matches!(
        read_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::StructureLimit)
    ));
    let mut small_row_budget = ResourceBudget::new(16 * 1024 * 1024, 8, 1_000_000);
    let page = read_table_page(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        TablePageRequest::new(17_998, 5).unwrap(),
        &mut small_row_budget,
        &|| false,
    )
    .unwrap();
    assert_eq!(page.schema().declared_rows(), 18_000);
    assert_eq!(page.rows(), expected(17_998, 2));
    assert!(page.reached_end());
}
#[test]
fn empty_out_of_range_and_invalid_requests_are_explicit() {
    for limit in [0, TablePageRequest::MAX_ROWS + 1, u32::MAX] {
        assert_eq!(
            TablePageRequest::new(0, limit),
            Err(InspectError::InvalidPageRange)
        );
    }
    for (count, offset) in [(0, 0), (3, 3), (3, u32::MAX)] {
        let page = read_table_page(
            &mut Cursor::new(database(count)),
            SCOPE,
            "Items",
            TablePageRequest::new(offset, 1).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(page.rows().is_empty());
        assert!(page.reached_end());
    }
}
#[test]
fn selected_relocations_and_deleted_roots_preserve_live_row_offsets() {
    let mut bytes = database(3);
    bytes.resize((FIRST_DATA_PAGE + 2) * PAGE_BYTES, 0);
    let pointer = (((FIRST_DATA_PAGE + 1) as u32) << 8).to_le_bytes().to_vec();
    let roots = [
        (0, row(1)),
        (0x8000, row(99)),
        (0x4000, pointer),
        (0, row(3)),
    ];
    bytes[FIRST_DATA_PAGE * PAGE_BYTES..(FIRST_DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_DEFINITION as u32, &roots));
    bytes[(FIRST_DATA_PAGE + 1) * PAGE_BYTES..]
        .copy_from_slice(&data_page(TABLE_DEFINITION as u32, &[(0x8000, row(2))]));
    let page = read_table_page(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        TablePageRequest::new(1, 2).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(page.rows(), expected(1, 2));
}
#[test]
fn malformed_selected_rows_and_visited_pages_fail_but_unvisited_payloads_are_not_audited() {
    let mut bytes = database(600);
    put32(&mut bytes, (FIRST_DATA_PAGE + 1) * PAGE_BYTES + 4, 999);
    assert!(
        read_table_page(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    assert!(matches!(
        read_table_page(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(200, 1).unwrap(),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
    let mut bytes = database(3);
    let row_start = usize::from(
        u16::from_le_bytes(
            bytes[FIRST_DATA_PAGE * PAGE_BYTES + 14..FIRST_DATA_PAGE * PAGE_BYTES + 16]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    put16(
        &mut bytes,
        FIRST_DATA_PAGE * PAGE_BYTES + row_start,
        u16::MAX,
    );
    assert!(
        read_table_page(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(
        read_table_page(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(1, 1).unwrap(),
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        expected(1, 1)
    );
    for declared in [0, 4] {
        let mut bytes = database(3);
        put32(
            &mut bytes,
            TABLE_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET,
            declared,
        );
        assert!(matches!(
            read_table_page(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                TablePageRequest::new(0, 10).unwrap(),
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Malformed)
        ));
    }
    let mut bytes = database(0);
    bytes[TABLE_DEFINITION * PAGE_BYTES + 63] = u8::MAX;
    assert!(matches!(
        read_table_page(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(100, 1).unwrap(),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
}
#[test]
fn cancellation_and_exhausted_budgets_do_not_publish_pages() {
    let bytes = database(3);
    let request = TablePageRequest::new(0, 1).unwrap();
    let calls = Cell::new(0);
    read_table_page(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        request,
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let final_call = calls.get();
    calls.set(0);
    assert!(matches!(
        read_table_page(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            request,
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == final_call
            }
        ),
        Err(InspectError::Cancelled)
    ));
    for mut budget in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100, 0),
    ] {
        assert!(matches!(
            read_table_page(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                request,
                &mut budget,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
}
proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(0x5041_4745), ..Default::default() })]
    #[test]
    fn pages_agree_with_an_independent_live_row_sequence(count in 0usize..650, offset in 0u32..700, limit in 1u32..40) {
        let bytes = database(count);
        let page = read_table_page(&mut Cursor::new(&bytes), SCOPE, "Items", TablePageRequest::new(offset, limit).unwrap(), &mut budget(), &|| false).unwrap();
        let start = (offset as usize).min(count);
        let returned = (count - start).min(limit as usize);
        prop_assert_eq!(page.rows(), expected(start, returned));
    }
}

#[path = "table_pages/snapshot_limits.rs"]
mod snapshot_limits;
