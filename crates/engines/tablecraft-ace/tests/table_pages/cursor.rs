use super::*;
use tablecraft_ace::TableCursor;

proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 64, rng_seed: proptest::test_runner::RngSeed::Fixed(0x4355_5253), ..Default::default() })]
    #[test]
    fn concatenated_batches_match_an_independent_row_sequence(count in 0usize..650, limit in 1u32..100) {
        let mut input = Cursor::new(database(count));
        let mut cursor = TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
        let mut offset = 0;
        loop {
            let batch = cursor.next_page(limit, &mut budget(), &|| false).unwrap();
            let returned = (count - offset).min(limit as usize);
            prop_assert_eq!(batch.offset(), offset as u32);
            prop_assert_eq!(batch.rows(), expected(offset, returned));
            offset += returned;
            if batch.reached_end() { break; }
        }
        prop_assert_eq!(offset, count);
    }
}

#[test]
fn continuation_handles_deleted_roots_and_relocations_between_batches() {
    let mut bytes = database(3);
    bytes.resize((FIRST_DATA_PAGE + 2) * PAGE_BYTES, 0);
    let pointer = (((FIRST_DATA_PAGE + 1) as u32) << 8).to_le_bytes().to_vec();
    bytes[FIRST_DATA_PAGE * PAGE_BYTES..(FIRST_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &data_page(
            TABLE_DEFINITION as u32,
            &[
                (0, row(1)),
                (0x8000, row(99)),
                (0x4000, pointer),
                (0, row(3)),
            ],
        ),
    );
    bytes[(FIRST_DATA_PAGE + 1) * PAGE_BYTES..]
        .copy_from_slice(&data_page(TABLE_DEFINITION as u32, &[(0x8000, row(2))]));
    let mut input = Cursor::new(bytes);
    let mut cursor =
        TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
    for offset in 0..3 {
        assert_eq!(
            cursor
                .next_page(1, &mut budget(), &|| false)
                .unwrap()
                .rows(),
            expected(offset, 1)
        );
    }
    assert!(
        cursor
            .next_page(1, &mut budget(), &|| false)
            .unwrap()
            .reached_end()
    );
}

#[test]
fn corrupt_later_usage_metadata_prevents_cursor_opening() {
    for indirect in [false, true] {
        let mut input = PageReads::new(map_fixture::database(6000, indirect, true));
        assert!(matches!(
            TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false),
            Err(InspectError::Malformed)
        ));
        assert_eq!(input.count(map_fixture::FIRST_DATA_PAGE as u64), 0);
    }
}

#[test]
fn batches_retain_the_current_page_and_check_eof_without_rescanning() {
    let mut input = PageReads::new(database(403));
    {
        let mut controls = budget();
        let mut cursor =
            TableCursor::open(&mut input, SCOPE, "Items", &mut controls, &|| false).unwrap();
        assert_eq!(cursor.columns().len(), 2);
        assert_eq!(cursor.schema().declared_rows(), 403);
        for (offset, limit, count, end) in [
            (0, 100, 100, false),
            (100, 101, 101, false),
            (201, 202, 202, false),
            (403, 1, 0, true),
            (403, 1, 0, true),
        ] {
            let batch = cursor.next_page(limit, &mut controls, &|| false).unwrap();
            assert_eq!(batch.offset(), offset as u32);
            assert_eq!(batch.rows(), expected(offset, count));
            assert_eq!(batch.reached_end(), end);
            assert_eq!(cursor.position(), (offset + count) as u32);
        }
    }
    for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + 3 {
        assert_eq!(input.count(page as u64), 1);
    }
}

#[test]
fn all_large_table_rows_can_be_fetched_with_explicit_per_batch_budgets() {
    for code in [1, 2, 5, 6] {
        for indirect in [false, true] {
            let mut bytes = map_fixture::database(6000, indirect, false);
            bytes[20] = code;
            if code != 1 {
                bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            }
            let mut input = PageReads::new(bytes);
            {
                let mut cursor =
                    TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false)
                        .unwrap();
                let mut offset = 0;
                loop {
                    let batch = cursor.next_page(250, &mut budget(), &|| false).unwrap();
                    assert_eq!(batch.offset(), offset as u32);
                    assert_eq!(batch.rows(), expected(offset, (6000 - offset).min(250)));
                    offset += batch.rows().len();
                    if batch.reached_end() {
                        break;
                    }
                }
                assert_eq!(offset, 6000);
            }
            for page in map_fixture::FIRST_DATA_PAGE..map_fixture::FIRST_DATA_PAGE + 6000 {
                assert_eq!(input.count(page as u64), 1);
            }
        }
    }
}

#[test]
fn invalid_ranges_leave_cursor_ready_but_failed_fetches_are_terminal() {
    let mut input = Cursor::new(database(3));
    let mut cursor =
        TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
    for limit in [0, TablePageRequest::MAX_ROWS + 1] {
        assert_eq!(
            cursor.next_page(limit, &mut budget(), &|| false).err(),
            Some(InspectError::InvalidPageRange)
        );
        assert!(!cursor.is_failed());
    }
    assert_eq!(
        cursor
            .next_page(1, &mut budget(), &|| false)
            .unwrap()
            .rows(),
        expected(0, 1)
    );
    let mut exhausted = ResourceBudget::new(16 * 1024 * 1024, 1, 1_000_000);
    assert!(matches!(
        cursor.next_page(2, &mut exhausted, &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(cursor.position(), 1);
    assert!(cursor.is_failed());
    assert_eq!(exhausted.remaining(Resource::Rows), 0);
    let called = Cell::new(false);
    assert_eq!(
        cursor
            .next_page(1, &mut budget(), &|| {
                called.set(true);
                false
            })
            .err(),
        Some(InspectError::CursorFailed)
    );
    assert!(!called.get());
}

#[test]
fn cached_pages_still_observe_cancellation_and_late_count_corruption() {
    for declared in [2, 4] {
        let mut bytes = database(3);
        put32(
            &mut bytes,
            TABLE_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET,
            declared,
        );
        let mut input = Cursor::new(bytes);
        let mut cursor =
            TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
        assert_eq!(
            cursor
                .next_page(2, &mut budget(), &|| false)
                .unwrap()
                .rows(),
            expected(0, 2)
        );
        assert_eq!(
            cursor.next_page(2, &mut budget(), &|| false).err(),
            Some(InspectError::Malformed)
        );
        assert_eq!(cursor.position(), 2);
        assert!(cursor.is_failed());
    }
    let mut input = Cursor::new(database(3));
    let mut cursor =
        TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
    cursor.next_page(1, &mut budget(), &|| false).unwrap();
    assert_eq!(
        cursor.next_page(1, &mut budget(), &|| true).err(),
        Some(InspectError::Cancelled)
    );
    assert!(cursor.is_failed());
    assert_eq!(cursor.position(), 1);
}
