use super::*;
use tablecraft_ace::{TableCursor, TableSnapshotLimits, read_table_batched};

fn larger_budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 50_000, 2_000_000)
}

#[test]
fn explicit_snapshot_limits_materialize_large_tables_without_changing_default_reads() {
    for code in [1, 2, 5, 6] {
        let mut bytes = database(20_000);
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let mut source = PageReads::new(bytes.clone());
        assert_eq!(
            read_table_batched(&mut source, SCOPE, "Items", &mut larger_budget(), &|| false).err(),
            Some(InspectError::StructureLimit)
        );
        assert_eq!(source.count(FIRST_DATA_PAGE as u64), 0);
        for batch in [1, 127, 5000] {
            let mut source = PageReads::new(bytes.clone());
            let mut budget = larger_budget();
            let cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut budget, &|| false).unwrap();
            let snapshot = cursor
                .into_snapshot(
                    TableSnapshotLimits::new(20_000, batch).unwrap(),
                    &mut budget,
                    &|| false,
                )
                .unwrap();
            assert_eq!(snapshot.rows(), expected(0, 20_000));
            for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + 100 {
                assert_eq!(
                    source.count(page as u64),
                    1,
                    "data pages must not be rescanned"
                );
            }
        }
    }
}

#[test]
fn snapshot_limits_are_checked_and_advanced_or_failed_cursors_cannot_publish_partial_tables() {
    for (rows, batch) in [(0, 1), (1_000_001, 1), (1, 0), (1, 16_385)] {
        assert_eq!(
            TableSnapshotLimits::new(rows, batch),
            Err(InspectError::InvalidSnapshotLimits)
        );
    }
    let limits = TableSnapshotLimits::new(3, 1).unwrap();
    let mut source = Cursor::new(database(3));
    let mut budget = larger_budget();
    let mut cursor =
        TableCursor::open(&mut source, SCOPE, "Items", &mut budget, &|| false).unwrap();
    let first = cursor.next_page(1, &mut budget, &|| false).unwrap();
    assert_eq!(
        cursor.into_snapshot(limits, &mut budget, &|| false).err(),
        Some(InspectError::CursorAdvanced)
    );
    assert_eq!(first.rows(), expected(0, 1));

    let mut source = Cursor::new(database(3));
    let mut cursor =
        TableCursor::open(&mut source, SCOPE, "Items", &mut larger_budget(), &|| false).unwrap();
    assert!(
        cursor
            .next_page(1, &mut ResourceBudget::new(1, 0, 1), &|| false)
            .is_err()
    );
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    assert_eq!(
        cursor
            .into_snapshot(limits, &mut larger_budget(), &cancelled)
            .err(),
        Some(InspectError::CursorFailed)
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn bounded_materialization_retains_projection_counts_and_all_cancellation_checkpoints() {
    let mut bytes = database(3);
    bytes[TABLE_DEFINITION * PAGE_BYTES + 63 + 25] = 0xff;
    let mut source = Cursor::new(bytes);
    let mut budget = larger_budget();
    let cursor =
        TableCursor::open_projection(&mut source, SCOPE, "Items", &["Id"], &mut budget, &|| false)
            .unwrap();
    assert_eq!(
        cursor
            .into_snapshot(
                TableSnapshotLimits::new(3, 1).unwrap(),
                &mut budget,
                &|| false
            )
            .unwrap()
            .rows(),
        &[
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );

    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        let mut source = Cursor::new(database(3));
        let mut budget = larger_budget();
        TableCursor::open(&mut source, SCOPE, "Items", &mut budget, &cancelled)?.into_snapshot(
            TableSnapshotLimits::new(3, 1).unwrap(),
            &mut budget,
            &cancelled,
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert_eq!(execute().err(), Some(InspectError::Cancelled));
    }
    let mut bytes = database(3);
    put32(
        &mut bytes,
        TABLE_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET,
        4,
    );
    let mut source = Cursor::new(bytes);
    let mut budget = larger_budget();
    let cursor = TableCursor::open(&mut source, SCOPE, "Items", &mut budget, &|| false).unwrap();
    assert_eq!(
        cursor
            .into_snapshot(
                TableSnapshotLimits::new(4, 1).unwrap(),
                &mut budget,
                &|| false
            )
            .err(),
        Some(InspectError::Malformed)
    );
}
