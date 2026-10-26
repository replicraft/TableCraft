use super::*;
use tablecraft_query::source::SourceKey;

#[test]
fn linked_and_external_leaves_keep_distinct_authorized_native_list_sources() {
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
        Cursor::new(fixture(6, 0)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let external_target = NativeSource::new_with_shared_budget(
        Cursor::new(fixture(6, 1000)),
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

    let external = ExternalNativeSource::with_links(
        &linked,
        &[NativeExternalMapping::new(
            "authorized",
            SourceKey::new(9),
            external_target.handle(),
        )],
    )
    .unwrap();

    let rows = execute(
        &external,
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Linked a \
        INNER JOIN [authorized].Items b ON a.Id<b.Id ORDER BY a.Id,b.Id",
    )
    .unwrap();

    assert_eq!(rows.len(), 3);
    for (row, (left, right)) in rows.iter().zip([(1, 2), (1, 3), (2, 3)]) {
        assert_pair(row, left, right, 1000);
    }
}

#[test]
fn scalar_only_join_leaf_never_reads_its_unselected_corrupt_child_payload() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(fixture(5, 0)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let rows_forbidden = Cell::new(true);
    let children_forbidden = Cell::new(true);
    let child_reads = Cell::new(0);
    let mut bad = fixture(5, 1000);
    bad[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let target = NativeSource::new_with_shared_budget(
        Watching {
            bytes: Cursor::new(bad),
            forbid_rows: &rows_forbidden,
            forbid_children: &children_forbidden,
            child_reads: &child_reads,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let facade = ExternalNativeSource::new(
        root.handle(),
        &[NativeExternalMapping::new(
            "scalar",
            SourceKey::new(8),
            target.handle(),
        )],
    )
    .unwrap();

    let sql = "SELECT a.Id,a.Score,b.Id FROM Items a \
        INNER JOIN [scalar].Items b ON a.Id=b.Id ORDER BY a.Id";
    let parsed = parse_query(sql, Default::default()).unwrap();

    let prepared = prepare(parsed.query(), &facade).unwrap();
    rows_forbidden.set(false);
    let result = prepared
        .execute_collections(&facade, &[], Default::default())
        .unwrap();

    assert_eq!(result.rows().len(), 3);
    for (position, row) in result.rows().iter().enumerate() {
        let id = position as i32 + 1;
        assert_eq!(scalar(&row[0]), Value::Int32(id));
        assert_eq!(list(&row[1]), members(id, 0));
        assert_eq!(scalar(&row[2]), Value::Int32(id));
    }
    assert_eq!(child_reads.get(), 0);
    children_forbidden.set(false);
    assert!(
        execute(
            &facade,
            "SELECT a.Score,b.Score FROM Items a \
        INNER JOIN [scalar].Items b ON a.Id=b.Id"
        )
        .is_err()
    );
    assert!(child_reads.get() > 0);
}
