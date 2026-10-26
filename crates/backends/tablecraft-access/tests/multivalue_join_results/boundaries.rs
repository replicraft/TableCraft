use super::*;

#[test]
fn unsupported_outer_and_list_predicates_refuse_before_native_records() {
    let forbid = Cell::new(true);
    let child_forbid = Cell::new(true);
    let reads = Cell::new(0);
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(fixture(5, 0)),
            forbid_rows: &forbid,
            forbid_children: &child_forbid,
            child_reads: &reads,
        },
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    for sql in [
        "SELECT a.Score,b.Score FROM Items a LEFT JOIN Items b ON a.Id=b.Id",
        "SELECT a.Score,b.Score.Value FROM Items a INNER JOIN Items b ON a.Id=b.Id",
        "SELECT a.Score FROM Items a INNER JOIN Items b ON a.Score=b.Score",
    ] {
        assert_eq!(
            execute(&source, sql).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(reads.get(), 0);
}

#[test]
fn cumulative_native_query_limits_and_cancellation_never_return_join_prefixes() {
    let cancel = Cell::new(false);
    let cancelled = || cancel.get();
    let pool = RefCell::new(allowance());
    let source = NativeSource::new_with_shared_budget(
        Cursor::new(fixture(6, 0)),
        CatalogScope::new(1),
        &pool,
        &cancelled,
    )
    .unwrap()
    .with_multivalues();

    let parsed = parse_query(
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
        ,Items b",
        Default::default(),
    )
    .unwrap();

    let prepared = prepare(parsed.query(), &source).unwrap();
    cancel.set(true);
    assert_eq!(
        prepared
            .execute_collections(&source, &[], Default::default())
            .err(),
        Some(QueryError::Cancelled)
    );
    cancel.set(false);
    let limited = ExecutionOptions {
        limits: tablecraft_query::QueryLimits {
            rows: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        prepared.execute_collections(&source, &[], limited),
        Err(QueryError::Limit(_))
    ));
    // A completed native cache remains valid after a query-budget failure.
    // A fresh source ensures this separate control actually performs native I/O.
    let fresh_pool = RefCell::new(allowance());
    let fresh_source = NativeSource::new_with_shared_budget(
        Cursor::new(fixture(6, 0)),
        CatalogScope::new(2),
        &fresh_pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let fresh_prepared = prepare(parsed.query(), &fresh_source).unwrap();
    let remaining = fresh_pool.borrow().remaining(Resource::Rows);
    fresh_pool
        .borrow_mut()
        .charge(Resource::Rows, remaining)
        .unwrap();
    assert!(matches!(
        fresh_prepared.execute_collections(&fresh_source, &[], Default::default()),
        Err(QueryError::Limit(_))
    ));
}
