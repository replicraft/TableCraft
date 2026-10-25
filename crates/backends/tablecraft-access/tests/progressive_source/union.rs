use super::*;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, PreparedQuery, StreamError, prepare};
use tablecraft_sql::parse_query;

const HEADER_MAGIC_START: usize = 4;
const FORMAT_CODE_OFFSET: usize = 20;
const ITEMS_DATA_PAGE: usize = 12;
const CATALOG_DEFINITION_PAGE: usize = 2;
const CATALOG_DATA_PAGE: usize = 4;
const OTHER_TABLE_PAGE: usize = 14;
const OTHER_USAGE_PAGE: usize = 15;
const OTHER_DATA_PAGE: usize = 16;

fn two_tables(profile: u8) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize((OTHER_DATA_PAGE + 1) * PAGE_BYTES, 0);
    bytes[FORMAT_CODE_OFFSET] = profile;
    if profile != 1 {
        bytes[HEADER_MAGIC_START..FORMAT_CODE_OFFSET].copy_from_slice(b"Standard ACE DB\0");
    }
    support::put32(
        &mut bytes,
        CATALOG_DEFINITION_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET,
        4,
    );
    let catalog = support::data_page(
        CATALOG_DEFINITION_PAGE as u32,
        &[
            (
                0,
                support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
            ),
            (
                0,
                support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, support::record(10, 0, 1, 0, b"\xff\xfeItems")),
            (0, support::record(14, 0, 1, 0, b"\xff\xfeOther")),
        ],
    );
    bytes[CATALOG_DATA_PAGE * PAGE_BYTES..(CATALOG_DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&catalog);
    let definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4)],
        2,
        (OTHER_USAGE_PAGE as u32) << 8,
    );
    bytes[OTHER_TABLE_PAGE * PAGE_BYTES..(OTHER_TABLE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    let usage = support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0, 1])]);
    bytes[OTHER_USAGE_PAGE * PAGE_BYTES..(OTHER_USAGE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&usage);
    let rows = [(41_i32, 410_i32), (42, 420)]
        .into_iter()
        .map(|(id, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    let data = support::data_page(OTHER_TABLE_PAGE as u32, &rows);
    bytes[OTHER_DATA_PAGE * PAGE_BYTES..(OTHER_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&data);
    bytes
}

fn plan(sql: &str, source: &dyn Source) -> PreparedQuery {
    let query = parse_query(sql, Default::default()).unwrap().into_query();
    prepare(&query, source).unwrap()
}

#[test]
fn successive_same_and_different_native_arms_release_cursors_and_preserve_originals() {
    for profile in [1, 2, 5, 6] {
        for batch in [1, 2, 5] {
            let bytes = two_tables(profile);
            let before = bytes.clone();
            let source = NativeSource::new_with_snapshot_limits(
                Cursor::new(bytes.as_slice()),
                SCOPE,
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, batch).unwrap(),
            )
            .unwrap();
            let query = plan(
                "SELECT Id FROM Items UNION ALL SELECT Id FROM Items UNION ALL SELECT Id FROM Other",
                &source,
            );
            let mut values = Vec::new();
            let summary = query
                .execute_progressive(&source, &[], ExecutionOptions::default(), &mut |row| {
                    values.push(row.to_vec());
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(summary.scanned_rows, 8);
            assert_eq!(summary.emitted_rows, 8);
            assert_eq!(
                values,
                [1, 2, 3, 1, 2, 3, 41, 42].map(|id| vec![Value::Int32(id)])
            );
            assert_eq!(
                source.scan_projected("Items", &[0], None, &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
            assert_eq!(
                source.scan_projected("Other", &[0], None, &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
            drop(source);
            assert_eq!(bytes, before);
        }
    }
}

#[test]
fn later_native_read_failure_stops_before_second_arm_and_retry_restarts_first() {
    let input = watched(two_tables(6));
    let failure = input.failed_page.clone();
    let reads = input.reads.clone();
    let source = source(input, 1);
    let query = plan(
        "SELECT Id FROM Items UNION ALL SELECT Id FROM Other",
        &source,
    );
    failure.set(Some(OTHER_DATA_PAGE));
    let mut values = Vec::new();
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
        values.push(row[0].clone());
        Ok::<(), ()>(())
    });
    assert!(matches!(error, Err(StreamError::Query(QueryError::Source))));
    assert_eq!(values, [Value::Int32(1), Value::Int32(2), Value::Int32(3)]);

    failure.set(None);
    let before = reads.get();
    let mut retried = Vec::new();
    query
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            retried.push(row[0].clone());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(retried, [1, 2, 3, 41, 42].map(Value::Int32));
    assert!(reads.get() > before);

    // A complete progressive retry must not publish a reusable snapshot either.
    failure.set(Some(ITEMS_DATA_PAGE));
    assert_eq!(collect(&source, &[0]), Err(QueryError::Source));
}

#[test]
fn late_expression_and_first_arm_eof_failures_never_start_the_following_arm() {
    let source = source(watched(two_tables(5)), 1);
    let query = plan(
        "SELECT Id FROM Items UNION ALL SELECT Id Mod (Id-42) FROM Other",
        &source,
    );
    let mut values = Vec::new();
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
        values.push(row[0].clone());
        Ok::<(), ()>(())
    });
    assert!(matches!(
        error,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(values, [1, 2, 3, 0].map(Value::Int64));

    let mut bytes = map_fixture::database(4, true, false);
    support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 5);
    let source = super::source(watched(bytes), 1);
    let query = plan(
        "SELECT Id FROM Items UNION ALL SELECT Score FROM Items",
        &source,
    );
    let mut values = Vec::new();
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
        values.push(row[0].clone());
        Ok::<(), ()>(())
    });
    assert!(matches!(error, Err(StreamError::Query(QueryError::Source))));
    assert_eq!(values, [1, 2, 3, 4].map(Value::Int32));
}

#[test]
fn cumulative_native_allowances_are_not_refilled_between_union_arms() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let pool = RefCell::new(budget());
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            watched(two_tables(6)),
            SCOPE,
            &pool,
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let query = plan(
            "SELECT Id FROM Items UNION ALL SELECT Id FROM Other",
            &source,
        );
        let mut values = Vec::new();
        let error = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
            values.push(row[0].clone());
            // Callback execution holds neither the native budget RefMut nor a
            // query-owned replacement pool. Exhaust the original host allowance.
            if values.len() == 3 {
                let mut allowance = pool.try_borrow_mut().unwrap();
                let remaining = allowance.remaining(resource);
                allowance.charge(resource, remaining).unwrap();
            }
            Ok::<(), ()>(())
        });
        assert!(matches!(
            error,
            Err(StreamError::Query(QueryError::Limit(_)))
        ));
        assert_eq!(values, [Value::Int32(1), Value::Int32(2), Value::Int32(3)]);
        assert_eq!(pool.borrow().remaining(resource), 0);
    }
}

#[test]
fn cancellation_and_sink_errors_release_cursor_borrows_without_promoting_caches() {
    let stopped = Cell::new(false);
    let cancelled = || stopped.get();
    let source = NativeSource::new_with_snapshot_limits(
        watched(two_tables(6)),
        SCOPE,
        budget(),
        &cancelled,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let query = plan(
        "SELECT Id FROM Items UNION ALL SELECT Id FROM Other",
        &source,
    );
    let mut count = 0;
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |_| {
        count += 1;
        assert_eq!(collect(&source, &[0]), Err(QueryError::Source));
        stopped.set(count == 3);
        Ok::<(), ()>(())
    });
    assert!(matches!(
        error,
        Err(StreamError::Query(QueryError::Cancelled))
    ));
    assert_eq!(count, 3);

    stopped.set(false);
    let mut count = 0;
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |_| {
        count += 1;
        Err("original sink rejection")
    });
    assert!(matches!(
        error,
        Err(StreamError::Sink("original sink rejection"))
    ));
    assert_eq!(count, 1);
    assert_eq!(collect(&source, &[0]).unwrap().len(), 3);
    assert_eq!(
        source.scan("Items", &mut |_| Ok(())),
        Err(QueryError::StructureLimit)
    );
}

#[test]
fn top_capped_native_arm_still_validates_late_pages_before_next_arm() {
    let input = watched(map_fixture::database(4, true, false));
    let failure = input.failed_page.clone();
    let source = source(input, 1);
    let query = plan(
        "SELECT TOP 1 Id FROM Items UNION ALL SELECT Score FROM Items",
        &source,
    );
    failure.set(Some(map_fixture::FIRST_DATA_PAGE + 3));
    let mut values = Vec::new();
    let error = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
        values.push(row[0].clone());
        Ok::<(), ()>(())
    });
    assert!(matches!(error, Err(StreamError::Query(QueryError::Source))));
    assert_eq!(values, [Value::Int32(1)]);

    failure.set(None);
    let mut retried = Vec::new();
    let summary = query
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            retried.push(row[0].clone());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(summary.scanned_rows, 8);
    assert_eq!(summary.emitted_rows, 5);
    assert_eq!(retried, [1, 10, 20, 30, 40].map(Value::Int32));
}
