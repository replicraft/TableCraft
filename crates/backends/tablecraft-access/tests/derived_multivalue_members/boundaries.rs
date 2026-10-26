use super::*;
use tablecraft_core::limits::Resource;
use tablecraft_query::source::SourceKey;

#[test]
fn derived_scalar_pruning_does_not_decode_corrupt_unselected_children() {
    let mut bytes = join_fixture::database(6, 0);
    bytes[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let source = NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let rows = execute(
        &source,
        "SELECT Q.Id FROM (SELECT Id FROM Items) Q ORDER BY Q.Id",
        false,
    );
    assert_eq!(rows.len(), 3);
    for (row, id) in rows.iter().zip(1..=3) {
        assert_eq!(scalar(&row[0]), &Value::Int32(id));
    }
    let syntax = parse_query(
        "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items) Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert!(
        plan.execute_collections(&source, &[], Default::default())
            .is_err()
    );
}

#[test]
fn derived_member_failures_cancellation_and_cumulative_limits_return_no_result() {
    let cancel = Cell::new(false);
    let cancelled = || cancel.get();
    let pool = RefCell::new(allowance());
    let source = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(1),
        &pool,
        &cancelled,
    )
    .unwrap()
    .with_multivalues();
    let syntax = parse_query(
        "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items) Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    cancel.set(true);
    assert_eq!(
        plan.execute_collections(&source, &[], Default::default())
            .err(),
        Some(QueryError::Cancelled)
    );
    cancel.set(false);
    let remaining = pool.borrow().remaining(Resource::Rows);
    pool.borrow_mut().charge(Resource::Rows, remaining).unwrap();
    assert!(matches!(
        plan.execute_collections(&source, &[], Default::default()),
        Err(QueryError::Limit(_))
    ));

    let fresh = NativeSource::new(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(2),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let syntax = parse_query(
        "SELECT Q.Whole,Q.Whole.Value FROM \
         (SELECT Score AS Whole,Id Mod (Id-2) AS Boom FROM Items) Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &fresh).unwrap();
    assert!(
        plan.execute_collections(&fresh, &[], Default::default())
            .is_err()
    );
}

#[test]
fn derived_lists_route_through_authorized_link_and_external_leaf_sources() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("opaque"),
            Some("Items"),
            None,
        )),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let linked_target = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let external_target = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::database(6, 1000)),
        CatalogScope::new(3),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let linked = LinkedNativeSource::new(
        root.handle(),
        &[NativeLinkMapping::new(
            "Linked",
            -1,
            "opaque",
            linked_target.handle(),
            "Items",
            10,
        )],
    )
    .unwrap();
    let source = ExternalNativeSource::with_links(
        &linked,
        &[NativeExternalMapping::new(
            "authorized",
            SourceKey::new(7),
            external_target.handle(),
        )],
    )
    .unwrap();
    for (table, offset) in [("Linked", 0), ("[authorized].Items", 1000)] {
        let sql = format!(
            "SELECT Q.Whole,Q.Whole.Value FROM \
             (SELECT Score AS Whole FROM {table} WHERE Id=1) Q"
        );
        let rows = execute(&source, &sql, false);
        assert_eq!(rows.len(), 4);
        for (row, one) in rows.iter().zip(members(1, false, offset)) {
            assert_eq!(list(&row[0]), members(1, false, offset));
            assert_eq!(scalar(&row[1]), &one);
        }
    }
}
