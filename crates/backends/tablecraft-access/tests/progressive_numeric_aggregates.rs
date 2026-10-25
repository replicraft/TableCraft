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
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use fixture as base;
use std::{cell::RefCell, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{
    limits::Resource,
    values::{Decimal, Double, Value},
};
use tablecraft_expressions::EvalError;
use tablecraft_query::{PreparedQuery, QueryError, Source, StreamError, prepare};
use tablecraft_sql::parse_query;

#[path = "progressive_counts/harness.rs"]
mod harness;
use harness::*;

const VALUES: [Option<i128>; 4] = [Some(10000), None, Some(20000), Some(30000)];
type CapturedStream = (Vec<Vec<Value>>, u64, u64);

fn decimal_database(profile: u8, values: &[Option<i128>], scale: u8) -> Vec<u8> {
    decimal_fixture::database_for(profile, false, 28, scale, values, 0)
}

fn native(bytes: Vec<u8>, maximum: u32, batch: u32) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        SCOPE,
        budget(),
        &|| false,
        TableSnapshotLimits::new(maximum, batch).unwrap(),
    )
    .unwrap()
}

fn plan(sql: &str, source: &dyn Source) -> PreparedQuery {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source).unwrap()
}

fn streamed(query: &PreparedQuery, source: &dyn Source) -> Result<CapturedStream, StreamError<()>> {
    let mut rows = Vec::new();
    let summary = query.execute_progressive(source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok((rows, summary.scanned_rows, summary.emitted_rows))
}

fn no_aggregate(query: &PreparedQuery, source: &dyn Source) -> StreamError<()> {
    let mut rows = Vec::new();
    let result = query.execute_progressive(source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    });
    assert!(rows.is_empty(), "incomplete aggregate result was published");
    result.unwrap_err()
}

fn assert_decimal(value: &Value, numerator: i128, denominator: i128) {
    let Value::Decimal(value) = value else {
        panic!("native Decimal aggregate lost its carrier");
    };
    assert_eq!(
        value.coefficient() * denominator,
        numerator * 10_i128.pow(u32::from(value.scale()))
    );
}

#[test]
fn native_decimal_and_integral_aggregates_finish_after_eof_without_snapshot_cache() {
    for profile in [1, 2, 5, 6] {
        for batch in [1, 2] {
            let bytes = decimal_database(profile, &VALUES, 4);
            let source = native(bytes.clone(), 1, batch);
            let buffered = native(bytes, 100, batch);
            let sql = concat!(
                "SELECT Count(*) AS N,Count(Score) AS C,",
                "Sum(Score) AS Total,Avg(Score) AS Mean,Sum(ID),Avg(ID) FROM Items",
            );
            let query = plan(sql, &source);
            let (rows, scanned, emitted) = streamed(&query, &source).unwrap();
            assert_eq!((scanned, emitted), (4, 1));
            assert_eq!(rows[0][..2], [Value::Int64(4), Value::Int64(3)]);
            assert_decimal(&rows[0][2], 6, 1);
            assert_decimal(&rows[0][3], 2, 1);
            assert_eq!(rows[0][4], Value::Int64(10));
            assert_eq!(rows[0][5], Value::Double(Double::from_float(2.5)));
            assert_eq!(
                rows,
                plan(sql, &buffered)
                    .execute(&buffered, &[], Default::default())
                    .unwrap()
                    .rows(),
            );
            assert_eq!(
                query.execute(&source, &[], Default::default()).err(),
                Some(QueryError::StructureLimit)
            );
            assert_eq!(streamed(&query, &source).unwrap().0, rows);
            assert_eq!(
                query.execute(&source, &[], Default::default()).err(),
                Some(QueryError::StructureLimit)
            );
        }
    }
}

#[test]
fn nullable_and_empty_native_operands_keep_null_totals_and_independent_denominators() {
    for profile in [1, 2, 5, 6] {
        for values in [vec![], vec![None, None], VALUES.to_vec()] {
            let source = native(decimal_database(profile, &values, 4), 1, 1);
            for filter in ["False", "Score IS NULL"] {
                let sql = format!(
                    "SELECT Count(*),Count(Score),Sum(Score),Avg(Score) FROM Items WHERE {filter}",
                );
                let expected_rows = if filter == "False" {
                    0
                } else {
                    values.iter().filter(|value| value.is_none()).count()
                };
                assert_eq!(
                    streamed(&plan(&sql, &source), &source).unwrap(),
                    (
                        vec![vec![
                            Value::Int64(expected_rows as i64),
                            Value::Int64(0),
                            Value::Null,
                            Value::Null
                        ]],
                        values.len() as u64,
                        1
                    ),
                );
            }
        }
    }
}

#[test]
fn tiny_native_decimal_residual_survives_wider_sum_and_mean_accumulation() {
    let maximum = 9_999_999_999_999_999_999_999_999_999_i128;
    let mut values = vec![Some(maximum); 9];
    values.push(Some(1));
    values.extend(vec![Some(-maximum); 9]);
    for profile in [1, 2, 5, 6] {
        let source = native(decimal_database(profile, &values, 28), 1, 1);
        let rows = streamed(
            &plan("SELECT Sum(Score),Avg(Score) FROM Items", &source),
            &source,
        )
        .unwrap();
        assert_eq!((rows.1, rows.2), (19, 1));
        assert_eq!(rows.0[0][0], Value::Decimal(Decimal::new(1, 28).unwrap()));
        // Exact mean is 1e-28 / 19, below half of the smallest Decimal unit.
        assert_decimal(&rows.0[0][1], 0, 1);
    }
}

#[test]
fn invalid_selected_decimal_refuses_but_integral_aggregate_projections_prune_it() {
    let mut bytes = decimal_database(6, &VALUES, 4);
    let directory = ORDINARY_DATA_PAGE * PAGE_BYTES + ROW_DIRECTORY_OFFSET + 3 * size_of::<u16>();
    let last = usize::from(u16::from_le_bytes(
        bytes[directory..directory + size_of::<u16>()]
            .try_into()
            .unwrap(),
    ));
    bytes[ORDINARY_DATA_PAGE * PAGE_BYTES + last + DECIMAL_VALUE_OFFSET] = 0x7f;
    let source = native(bytes, 1, 1);
    let ids = plan("SELECT Sum(ID),Avg(ID),Count(*) FROM Items", &source);
    assert_eq!(
        streamed(&ids, &source).unwrap().0,
        [vec![
            Value::Int64(10),
            Value::Double(Double::from_float(2.5)),
            Value::Int64(4)
        ]]
    );
    for sql in [
        "SELECT Sum(Score) FROM Items",
        "SELECT Avg(Score) FROM Items",
        "SELECT Sum(ID),Avg(Score) FROM Items",
    ] {
        assert!(matches!(
            no_aggregate(&plan(sql, &source), &source),
            StreamError::Query(QueryError::Unsupported)
        ));
    }
    assert_eq!(streamed(&ids, &source).unwrap().1, 4);
}

#[test]
fn late_native_failures_never_publish_numeric_aggregates_or_activate_later_arms() {
    for failure in ["page", "count", "io"] {
        let (mut bytes, other_data) = with_other(database(6, 4));
        if failure == "page" {
            bytes[(FIRST_DATA_PAGE + 3) * PAGE_BYTES] = 0;
        } else if failure == "count" {
            support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 5);
        }
        let watched = Watched::new(bytes);
        let pages = watched.pages.clone();
        if failure == "io" {
            watched.failed_page.set(Some(FIRST_DATA_PAGE + 3));
        }
        let source = source(watched, &|| false);
        let query = plan(
            "SELECT Sum(Score),Avg(Score) FROM Items UNION ALL SELECT Sum(Score),Avg(Score) FROM Other",
            &source,
        );
        assert!(matches!(
            no_aggregate(&query, &source),
            StreamError::Query(QueryError::Source)
        ));
        assert!(!pages.borrow().contains(&other_data));
    }
}

#[test]
fn late_eager_aggregate_operands_fail_before_publication_and_retry_starts_clean() {
    let (bytes, other_data) = with_other(database(5, 4));
    let watched = Watched::new(bytes);
    let pages = watched.pages.clone();
    let source = source(watched, &|| false);
    let sql = concat!(
        "SELECT Sum(IIf(True,ID,1\\(ID-4))),Avg(Score) FROM Items UNION ALL ",
        "SELECT Sum(ID),Avg(Score) FROM Other",
    );
    assert!(matches!(
        no_aggregate(&plan(sql, &source), &source),
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    assert!(!pages.borrow().contains(&other_data));
    let rows = streamed(
        &plan("SELECT Sum(Score),Avg(Score) FROM Items", &source),
        &source,
    )
    .unwrap();
    assert_eq!(
        rows,
        (
            vec![vec![
                Value::Int64(100),
                Value::Double(Double::from_float(25.0))
            ]],
            4,
            1
        )
    );
}

#[test]
fn numeric_aggregate_arms_share_cumulative_native_limits_and_stop_before_later_inputs() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let (bytes, other_data) = with_other(database(6, 4));
        let watched = Watched::new(bytes);
        let pages = watched.pages.clone();
        let pool = RefCell::new(budget());
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            watched,
            SCOPE,
            &pool,
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let query = plan(
            "SELECT Sum(ID) FROM Items UNION ALL SELECT Sum(ID) FROM Other",
            &source,
        );
        let mut rows = Vec::new();
        let failure = query.execute_progressive(&source, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            let mut native_budget = pool.try_borrow_mut().unwrap();
            let allowance = native_budget.remaining(resource);
            native_budget.charge(resource, allowance).unwrap();
            Ok::<(), ()>(())
        });
        assert!(matches!(
            failure,
            Err(StreamError::Query(QueryError::Limit(_)))
        ));
        assert_eq!(rows, [vec![Value::Int64(10)]]);
        assert!(!pages.borrow().contains(&other_data));
    }
}

#[test]
fn numeric_aggregates_honor_cancellation_and_ignore_previous_complete_snapshots() {
    let watched = Watched::new(database(6, 4));
    let stopped = watched.stopped.clone();
    watched.cancel_page.set(Some(FIRST_DATA_PAGE + 3));
    let cancel = || stopped.get();
    let source = source(watched, &cancel);
    let query = plan("SELECT Sum(Score),Avg(Score) FROM Items", &source);
    assert!(matches!(
        no_aggregate(&query, &source),
        StreamError::Query(QueryError::Cancelled)
    ));

    let watched = Watched::new(database(6, 4));
    let failed_page = watched.failed_page.clone();
    let source = NativeSource::new_with_snapshot_limits(
        watched,
        SCOPE,
        budget(),
        &|| false,
        TableSnapshotLimits::new(100, 1).unwrap(),
    )
    .unwrap();
    let query = plan("SELECT Sum(Score),Avg(Score) FROM Items", &source);
    let expected = query
        .execute(&source, &[], Default::default())
        .unwrap()
        .rows()
        .to_vec();
    failed_page.set(Some(FIRST_DATA_PAGE + 3));
    assert!(matches!(
        no_aggregate(&query, &source),
        StreamError::Query(QueryError::Source)
    ));
    assert_eq!(
        query
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        expected
    );
    failed_page.set(None);
    assert_eq!(streamed(&query, &source).unwrap().0, expected);
}
