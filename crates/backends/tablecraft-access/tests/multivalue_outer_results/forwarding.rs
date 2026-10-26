use super::*;
use tablecraft_query::source::SourceKey;

#[test]
fn authorized_link_and_external_outer_leaves_keep_target_lists_and_absence() {
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

    let left = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::with_null_parent(6)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let right = NativeSource::new_with_shared_budget(
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
            left.handle(),
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
            right.handle(),
        )],
    )
    .unwrap();

    let rows = execute(
        &source,
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Linked a \
        LEFT JOIN [authorized].Items b ON a.Id=b.Id+1 ORDER BY a.Id",
    )
    .unwrap();

    assert_eq!(rows.len(), 3);
    for (position, row) in rows.iter().enumerate() {
        let id = position as i32 + 1;
        assert_eq!(scalar(&row[0]), Value::Int32(id));
        assert_eq!(list(&row[2]), expected(Some(id)).unwrap());
        if id == 1 {
            assert_eq!(scalar(&row[1]), Value::Null);
            absent(&row[3]);
        } else {
            assert_eq!(scalar(&row[1]), Value::Int32(id - 1));
            assert_eq!(list(&row[3]), members(id - 1, 1000));
        }
    }
}

#[test]
fn scalar_only_nullable_leaf_prunes_corrupt_children_but_selected_list_fails() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::with_null_parent(6)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let forbidden = Cell::new(true);
    let children = Cell::new(true);
    let reads = Cell::new(0);
    let mut bad = fixture(6, 1000);
    bad[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let target = NativeSource::new_with_shared_budget(
        Watching {
            bytes: Cursor::new(bad),
            forbid_rows: &forbidden,
            forbid_children: &children,
            child_reads: &reads,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let source = ExternalNativeSource::new(
        root.handle(),
        &[NativeExternalMapping::new(
            "scalar",
            SourceKey::new(8),
            target.handle(),
        )],
    )
    .unwrap();

    let parsed = parse_query(
        "SELECT a.Id,b.Id,a.Score FROM Items a \
        LEFT JOIN [scalar].Items b ON a.Id=b.Id+1 ORDER BY a.Id",
        Default::default(),
    )
    .unwrap();

    let prepared = prepare(parsed.query(), &source).unwrap();
    forbidden.set(false);
    let rows = prepared
        .execute_collections(&source, &[], Default::default())
        .unwrap();

    assert_eq!(rows.rows().len(), 3);
    for (position, row) in rows.rows().iter().enumerate() {
        let id = position as i32 + 1;
        assert_eq!(scalar(&row[0]), Value::Int32(id));
        assert_eq!(
            scalar(&row[1]),
            if id == 1 {
                Value::Null
            } else {
                Value::Int32(id - 1)
            }
        );
        assert_eq!(list(&row[2]), expected(Some(id)).unwrap());
    }
    assert_eq!(reads.get(), 0);
    children.set(false);
    assert!(
        execute(
            &source,
            "SELECT a.Score,b.Score FROM Items a \
        LEFT JOIN [scalar].Items b ON a.Id=b.Id+1"
        )
        .is_err()
    );
    assert!(reads.get() > 0);
}
