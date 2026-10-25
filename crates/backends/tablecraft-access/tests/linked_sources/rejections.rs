use super::*;

#[test]
fn owned_or_different_allowances_and_scope_collisions_are_refused() {
    let pool = RefCell::new(budget());
    let other = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &mappings(&target)),
        Err(LinkSourceError::SharedBudgetRequired)
    ));
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        &other,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &mappings(&target)),
        Err(LinkSourceError::SharedBudgetRequired)
    ));
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &mappings(&target)),
        Err(LinkSourceError::ScopeCollision)
    ));
}

#[test]
fn provider_connections_odbc_and_link_chains_do_not_become_local_targets() {
    for (kind, connect) in [(6, Some("ODBC;anything")), (4, None)] {
        let pool = RefCell::new(budget());
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(linked_fixture::database(
                kind,
                Some("opaque-backend"),
                Some("Items"),
                connect,
            )),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget(
            Cursor::new(target_fixture::database()),
            CatalogScope::new(2),
            &pool,
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            LinkedNativeSource::new(root.handle(), &mappings(&target)),
            Err(LinkSourceError::UnsupportedTarget)
        ));
    }
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("opaque-backend"),
            Some("Linked"),
            None,
        )),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let mapping = NativeLinkMapping::new(
        "Linked",
        -1,
        "opaque-backend",
        target.handle(),
        "Linked",
        -1,
    );
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &[mapping]),
        Err(LinkSourceError::UnsupportedTarget)
    ));
}

#[test]
fn cancelled_or_exhausted_lookup_and_setup_never_refill_the_native_allowance() {
    let pool = RefCell::new(budget());
    let stop = Cell::new(false);
    let cancelled = || stop.get();
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &cancelled,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        &pool,
        &cancelled,
    )
    .unwrap();
    stop.set(true);
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &mappings(&target)),
        Err(LinkSourceError::Query(QueryError::Cancelled))
    ));
    stop.set(false);
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    stop.set(true);
    assert!(matches!(
        facade.columns("Linked"),
        Err(QueryError::Cancelled)
    ));
    stop.set(false);
    let steps = pool.borrow().remaining(Resource::Steps);
    pool.borrow_mut().charge(Resource::Steps, steps).unwrap();
    assert!(
        matches!(facade.columns("Linked"), Err(QueryError::Limit(limit)) if limit.resource == Resource::Steps)
    );
    assert_eq!(pool.borrow().remaining(Resource::Steps), 0);
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &mappings(&target)),
        Err(LinkSourceError::Native(
            tablecraft_ace::InspectError::Limit(_)
        ))
    ));
}
