use super::*;

#[test]
fn legacy_scalar_publication_refuses_before_records_and_budgets_remain_separate() {
    let forbidden = Cell::new(true);
    let children = Cell::new(true);
    let reads = Cell::new(0);
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let pool = RefCell::new(allowance());
    let source = NativeSource::new_with_shared_budget(
        Watching {
            bytes: Cursor::new(join_fixture::with_null_parent(6)),
            forbid_rows: &forbidden,
            forbid_children: &children,
            child_reads: &reads,
        },
        CatalogScope::new(1),
        &pool,
        &callback,
    )
    .unwrap()
    .with_multivalues();

    let parsed = parse_query(
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
        LEFT JOIN Items b ON a.Id=b.Id+1",
        Default::default(),
    )
    .unwrap();

    let prepared = prepare(parsed.query(), &source).unwrap();

    assert_eq!(
        prepared.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(reads.get(), 0);
    cancelled.set(true);
    assert_eq!(
        prepared
            .execute_collections(&source, &[], Default::default())
            .err(),
        Some(QueryError::Cancelled)
    );
    cancelled.set(false);
    forbidden.set(false);
    children.set(false);
    let remaining = pool.borrow().remaining(Resource::Rows);
    pool.borrow_mut().charge(Resource::Rows, remaining).unwrap();

    assert!(matches!(
        prepared.execute_collections(&source, &[], Default::default()),
        Err(QueryError::Limit(_))
    ));

    let fresh = NativeSource::new(
        Cursor::new(join_fixture::with_null_parent(6)),
        CatalogScope::new(2),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let prepared = prepare(parsed.query(), &fresh).unwrap();

    let options = ExecutionOptions {
        limits: tablecraft_query::QueryLimits {
            rows: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        prepared.execute_collections(&fresh, &[], options),
        Err(QueryError::Limit(_))
    ));
}

#[test]
fn eager_error_after_valid_outer_rows_returns_no_collection_result() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::with_null_parent(6)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let error = execute(
        &source,
        "SELECT a.Id,a.Score,a.Id Mod (b.Id-2) AS Boom \
        FROM Items a LEFT JOIN Items b ON a.Id=b.Id+1",
    )
    .unwrap_err();
    assert!(matches!(error, QueryError::Evaluation(_)));
    assert!(error.to_string().contains("division by zero"));
}
