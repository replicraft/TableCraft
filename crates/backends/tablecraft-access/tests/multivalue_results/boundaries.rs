use super::*;

#[test]
fn scalar_publication_and_unsupported_list_operations_refuse_before_any_records() {
    let forbid = Cell::new(true);
    let fail = Cell::new(false);
    let reads = Cell::new(0);
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(populated(5)),
            forbid_rows: &forbid,
            fail_children: &fail,
            child_reads: &reads,
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let parsed = parse_query("SELECT Id,Score FROM Items", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    let tablecraft_query::OutputType::Multivalue {
        element_type,
        element_nullable,
        parent_nullable: false,
    } = prepared.output_types().unwrap()[1]
    else {
        panic!("Bare collection output lost its element descriptor");
    };
    assert_eq!(
        element_type.scalar_type(),
        tablecraft_schema::ScalarType::Int32
    );
    assert!(element_nullable);
    assert_eq!(
        prepared.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    for sql in [
        "SELECT DISTINCT Score FROM Items",
        "SELECT Score FROM Items ORDER BY Score",
        "SELECT Score FROM Items WHERE Score IS NULL",
        "SELECT DISTINCT Score,Score.Value FROM Items",
        "SELECT Score FROM Items UNION ALL SELECT Score FROM Items",
    ] {
        assert_eq!(
            query(&source, sql).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(reads.get(), 0);
}

#[test]
fn failed_child_reads_leave_the_collection_unpublished_and_retryable() {
    let forbid = Cell::new(false);
    let fail = Cell::new(true);
    let reads = Cell::new(0);
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(populated(6)),
            forbid_rows: &forbid,
            fail_children: &fail,
            child_reads: &reads,
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let parsed = parse_query("SELECT Count(*) FROM Items", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(reads.get(), 0);
    assert!(query(&source, "SELECT Id,Score FROM Items").is_err());
    let failed_reads = reads.get();
    assert!(failed_reads > 0);
    fail.set(false);
    assert_eq!(
        observe(&query(&source, "SELECT Id,Score FROM Items").unwrap()),
        [
            (1, vec![Value::Int32(100), Value::Int32(101)]),
            (2, vec![Value::Int32(200)]),
            (3, vec![]),
        ]
    );
    assert!(reads.get() > failed_reads);
}

#[test]
fn cancellation_and_independent_native_query_limits_return_no_collection_result() {
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let source = NativeSource::new(
        Cursor::new(populated(5)),
        CatalogScope::new(1),
        budget(),
        &callback,
    )
    .unwrap()
    .with_multivalues();
    let parsed = parse_query("SELECT Id,Score FROM Items", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    cancelled.set(true);
    assert_eq!(
        prepared
            .execute_collections(&source, &[], Default::default())
            .err(),
        Some(QueryError::Cancelled),
    );
    cancelled.set(false);
    let options = ExecutionOptions {
        limits: tablecraft_query::QueryLimits {
            rows: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        prepared.execute_collections(&source, &[], options),
        Err(QueryError::Limit(_)),
    ));
    assert_eq!(
        observe(&query(&source, "SELECT Id,Score FROM Items").unwrap()).len(),
        3
    );

    let pool = RefCell::new(budget());
    let source = NativeSource::new_with_shared_budget(
        Cursor::new(populated(5)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let prepared = prepare(parsed.query(), &source).unwrap();
    let remaining = pool
        .borrow()
        .remaining(tablecraft_core::limits::Resource::Rows);
    pool.borrow_mut()
        .charge(tablecraft_core::limits::Resource::Rows, remaining)
        .unwrap();
    assert!(matches!(
        prepared.execute_collections(&source, &[], Default::default()),
        Err(QueryError::Limit(_)),
    ));
}

#[test]
fn native_opt_in_and_missing_required_index_refuse_before_bare_output_reads() {
    let source = NativeSource::new(
        Cursor::new(populated(6)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(query(&source, "SELECT Id,Score FROM Items").is_err());

    let forbid = Cell::new(true);
    let fail = Cell::new(false);
    let reads = Cell::new(0);
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(populated(6)),
            forbid_rows: &forbid,
            fail_children: &fail,
            child_reads: &reads,
        },
        CatalogScope::new(2),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues()
    .with_index("Items", "Missing")
    .unwrap();
    // Indexed collections are supported; the older blanket Unsupported
    // expectation is replaced by the precise missing required-index failure.
    assert_eq!(
        query(&source, "SELECT Id,Score FROM Items").err(),
        Some(QueryError::UnknownName),
    );
    assert_eq!(reads.get(), 0);
}
