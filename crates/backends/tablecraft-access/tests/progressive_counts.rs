#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/decimal_indexes/fixture.rs"]
mod decimal_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/large_maps/fixture.rs"]
mod map_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use fixture as base;
use std::{cell::RefCell, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, PreparedQuery, QueryError, Source, StreamError, prepare};
use tablecraft_sql::parse_query;

#[path = "progressive_counts/harness.rs"]
mod harness;
use harness::*;

#[path = "progressive_counts/operands.rs"]
mod operands;

type CapturedStream = (Vec<Vec<Value>>, u64, u64);

fn plan(sql: &str, source: &dyn Source) -> PreparedQuery {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source).unwrap()
}

fn streamed(query: &PreparedQuery, source: &dyn Source) -> Result<CapturedStream, StreamError<()>> {
    let mut rows = Vec::new();
    let summary = query.execute_progressive(source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok((rows, summary.scanned_rows, summary.emitted_rows))
}

fn refused_without_count(query: &PreparedQuery, source: &dyn Source) -> StreamError<()> {
    let mut rows = Vec::new();
    let result = query.execute_progressive(source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    });
    assert!(
        rows.is_empty(),
        "an incomplete Count aggregate was published"
    );
    result.unwrap_err()
}

#[test]
fn count_reads_past_snapshot_retention_and_publishes_only_the_complete_aggregate() {
    for profile in [1, 2, 5, 6] {
        for batch in [1, 2] {
            let bytes = database(profile, 4);
            let original = bytes.clone();
            let native = NativeSource::new_with_snapshot_limits(
                Cursor::new(bytes.as_slice()),
                SCOPE,
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, batch).unwrap(),
            )
            .unwrap();

            for (sql, expected) in [
                ("SELECT Count(*) FROM Items", 4),
                ("SELECT TOP 1 Count(*) FROM Items", 4),
                ("SELECT Count(*) FROM Items WHERE Score>20", 2),
                ("SELECT Count(*) FROM Items WHERE Score>100", 0),
            ] {
                let query = plan(sql, &native);
                assert_eq!(
                    streamed(&query, &native).unwrap(),
                    (vec![vec![Value::Int64(expected)]], 4, 1),
                );
            }

            let query = plan("SELECT Count(*) FROM Items", &native);
            assert_eq!(
                query.execute(&native, &[], Default::default()).err(),
                Some(QueryError::StructureLimit),
            );
            assert_eq!(
                streamed(&query, &native).unwrap().0,
                [vec![Value::Int64(4)]]
            );
            assert_eq!(
                query.execute(&native, &[], Default::default()).err(),
                Some(QueryError::StructureLimit),
            );
            drop(native);
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn zero_width_counts_prune_bad_unselected_values_but_required_filters_refuse_them() {
    for profile in [1, 2, 5, 6] {
        let mut bytes =
            decimal_fixture::database_for(profile, false, 28, 4, &[Some(1), Some(2), Some(3)], 0);
        let first_row = u16::from_le_bytes(
            bytes[ORDINARY_DATA_PAGE * PAGE_BYTES + ROW_DIRECTORY_OFFSET
                ..ORDINARY_DATA_PAGE * PAGE_BYTES + ROW_DIRECTORY_OFFSET + size_of::<u16>()]
                .try_into()
                .unwrap(),
        ) as usize;
        bytes[ORDINARY_DATA_PAGE * PAGE_BYTES + first_row + DECIMAL_VALUE_OFFSET] = 0x7f;
        let native = NativeSource::new_with_snapshot_limits(
            Cursor::new(bytes),
            SCOPE,
            budget(),
            &|| false,
            TableSnapshotLimits::new(3, 1).unwrap(),
        )
        .unwrap();

        for sql in [
            "SELECT Count(*) FROM Items",
            "SELECT Count(*) FROM Items WHERE Id>0",
        ] {
            assert_eq!(
                streamed(&plan(sql, &native), &native).unwrap(),
                (vec![vec![Value::Int64(3)]], 3, 1),
            );
        }
        assert_eq!(
            plan("SELECT Score FROM Items", &native)
                .execute(&native, &[], Default::default())
                .err(),
            Some(QueryError::Unsupported),
        );
        assert!(matches!(
            refused_without_count(
                &plan("SELECT Count(*) FROM Items WHERE Score=Score", &native),
                &native
            ),
            StreamError::Query(QueryError::Unsupported),
        ));
    }
}

#[test]
fn late_native_page_count_and_io_failures_publish_no_count_and_never_start_later_arms() {
    for failure in ["page", "count", "io"] {
        let (mut bytes, other_data) = with_other(database(6, 4));
        if failure == "page" {
            bytes[(FIRST_DATA_PAGE + 3) * PAGE_BYTES] = 0;
        } else if failure == "count" {
            support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 5);
        }
        let input = Watched::new(bytes);
        let observed = input.pages.clone();
        if failure == "io" {
            input.failed_page.set(Some(FIRST_DATA_PAGE + 3));
        }
        let native = source(input, &|| false);
        let query = plan(
            "SELECT Count(*) FROM Items UNION ALL SELECT Count(*) FROM Other",
            &native,
        );
        let mut rows = Vec::new();
        let error = query.execute_progressive(&native, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        });

        assert!(matches!(error, Err(StreamError::Query(QueryError::Source))));
        assert!(rows.is_empty());
        let visited = observed.borrow();
        for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + 3 {
            assert!(visited.contains(&page));
        }
        assert!(!visited.contains(&other_data));
    }
}

#[test]
fn late_eager_filter_errors_discard_the_count_and_retries_have_no_cached_prefix() {
    let (bytes, other_data) = with_other(database(5, 4));
    let input = Watched::new(bytes);
    let pages = input.pages.clone();
    let failure = input.failed_page.clone();
    let native = source(input, &|| false);
    let query = plan(
        "SELECT Count(*) FROM Items WHERE Id Mod (Id-4)=0 UNION ALL SELECT Count(*) FROM Other",
        &native,
    );
    assert!(matches!(
        refused_without_count(&query, &native),
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero)),
    ));
    assert!(!pages.borrow().contains(&other_data));

    let count = plan("SELECT Count(*) FROM Items", &native);
    assert_eq!(
        streamed(&count, &native).unwrap().0,
        [vec![Value::Int64(4)]]
    );
    failure.set(Some(FIRST_DATA_PAGE + 3));
    assert!(matches!(
        refused_without_count(&count, &native),
        StreamError::Query(QueryError::Source)
    ));
    failure.set(None);
    assert_eq!(
        streamed(&count, &native).unwrap().0,
        [vec![Value::Int64(4)]]
    );
}

#[test]
fn aggregate_publication_releases_native_budgets_and_later_arms_never_refill_them() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        for before_first in [false, true] {
            let (bytes, other_data) = with_other(database(6, 4));
            let input = Watched::new(bytes);
            let pages = input.pages.clone();
            let pool = RefCell::new(budget());
            let native = NativeSource::new_with_shared_budget_and_snapshot_limits(
                input,
                SCOPE,
                &pool,
                &|| false,
                TableSnapshotLimits::new(1, 1).unwrap(),
            )
            .unwrap();
            let query = plan(
                "SELECT Count(*) FROM Items UNION ALL SELECT Count(*) FROM Other",
                &native,
            );
            if before_first {
                let mut remaining = pool.borrow_mut();
                let allowance = remaining.remaining(resource);
                remaining.charge(resource, allowance).unwrap();
            }

            let mut rows = Vec::new();
            let error = query.execute_progressive(&native, &[], Default::default(), &mut |row| {
                rows.push(row.to_vec());
                let mut remaining = pool.try_borrow_mut().unwrap();
                let allowance = remaining.remaining(resource);
                remaining.charge(resource, allowance).unwrap();
                Ok::<(), ()>(())
            });

            assert!(matches!(
                error,
                Err(StreamError::Query(QueryError::Limit(_)))
            ));
            let expected = if before_first {
                vec![]
            } else {
                vec![vec![Value::Int64(4)]]
            };
            assert_eq!(rows, expected);
            assert!(!pages.borrow().contains(&other_data));
            assert_eq!(pool.borrow().remaining(resource), 0);
        }
    }
}

#[test]
fn late_cancellation_and_sink_rejection_publish_no_incomplete_count_or_poisoned_retry() {
    let input = Watched::new(database(6, 4));
    let stopped = input.stopped.clone();
    input.cancel_page.set(Some(FIRST_DATA_PAGE + 3));
    let cancel = || stopped.get();
    let native = source(input, &cancel);
    let query = plan("SELECT Count(*) FROM Items", &native);
    assert!(matches!(
        refused_without_count(&query, &native),
        StreamError::Query(QueryError::Cancelled)
    ));

    let native = source(Watched::new(database(6, 4)), &|| false);
    let query = plan(
        "SELECT Count(*) FROM Items UNION ALL SELECT Count(*) FROM Items",
        &native,
    );
    let mut visits = 0;
    let error = query.execute_progressive(&native, &[], ExecutionOptions::default(), &mut |_| {
        visits += 1;
        Err("original sink failure")
    });
    assert!(matches!(
        error,
        Err(StreamError::Sink("original sink failure"))
    ));
    assert_eq!(visits, 1);
    assert_eq!(
        streamed(&query, &native).unwrap(),
        (vec![vec![Value::Int64(4)], vec![Value::Int64(4)]], 8, 2),
    );
}

#[test]
fn supported_integral_grouped_counts_stream_past_snapshot_retention() {
    let input = Watched::new(database(6, 4));
    let pages = input.pages.clone();
    let native = source(input, &|| false);
    let query = plan("SELECT Score,Count(*) FROM Items GROUP BY Score", &native);
    assert_eq!(
        streamed(&query, &native).unwrap(),
        (
            vec![
                vec![Value::Int32(10), Value::Int64(1)],
                vec![Value::Int32(20), Value::Int64(1)],
                vec![Value::Int32(30), Value::Int64(1)],
                vec![Value::Int32(40), Value::Int64(1)],
            ],
            4,
            4,
        )
    );
    for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + 4 {
        assert!(pages.borrow().contains(&page));
    }
    assert_eq!(
        query.execute(&native, &[], Default::default()).err(),
        Some(QueryError::StructureLimit),
    );
}

#[test]
fn unsupported_count_forms_and_required_indexes_never_fall_back_to_snapshots() {
    let sql = "SELECT Score,Count(*) FROM Items GROUP BY Score ORDER BY Score";
    let input = Watched::new(database(6, 4));
    let pages = input.pages.clone();
    let native = source(input, &|| false);
    assert!(matches!(
        refused_without_count(&plan(sql, &native), &native),
        StreamError::Query(QueryError::Unsupported)
    ));
    assert!(
        !pages
            .borrow()
            .iter()
            .any(|page| (FIRST_DATA_PAGE..FIRST_DATA_PAGE + 4).contains(page))
    );

    let native = source(Watched::new(database(6, 4)), &|| false)
        .with_index("Items", "MissingIndex")
        .unwrap();
    assert!(matches!(
        refused_without_count(&plan("SELECT Count(*) FROM Items", &native), &native),
        StreamError::Query(QueryError::Unsupported),
    ));
}
