use super::*;
use tablecraft_access::LinkSourceError;
use tablecraft_expressions::EvalError;
use tablecraft_query::StreamError;

const DECLARED_ROWS_OFFSET: usize = 16;

#[test]
fn shared_native_limits_and_cancellation_stop_the_mapped_cursor() {
    for resource in [
        None,
        Some(Resource::Bytes),
        Some(Resource::Rows),
        Some(Resource::Steps),
    ] {
        let pool = RefCell::new(budget());
        let cancelled = Cell::new(false);
        let cancellation = || cancelled.get();
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(root_bytes(6)),
            CatalogScope::new(1),
            &pool,
            &cancellation,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget_and_snapshot_limits(
            Cursor::new(target_fixture::database()),
            CatalogScope::new(2),
            &pool,
            &cancellation,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
        let mut visits = 0;
        let result = facade.scan_projected_progressive("Linked", &[0], &mut |_| {
            visits += 1;
            if let Some(resource) = resource {
                let remaining = pool.borrow().remaining(resource);
                pool.borrow_mut().charge(resource, remaining).unwrap();
            } else {
                cancelled.set(true);
            }
            Ok(())
        });
        assert_eq!(visits, 1);
        if let Some(resource) = resource {
            assert!(matches!(result, Err(QueryError::Limit(limit)) if limit.resource == resource));
            assert_eq!(pool.borrow().remaining(resource), 0);
        } else {
            assert_eq!(result, Err(QueryError::Cancelled));
            cancelled.set(false);
            assert_eq!(collect(&facade, &[0]).unwrap().len(), 3);
        }
    }
}

#[test]
fn mapped_target_options_remain_explicit_refusals_without_sequential_fallback() {
    for mode in 0..4 {
        let pool = RefCell::new(budget());
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(root_bytes(6)),
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
        let target = match mode {
            0 => target.with_index("Items", "MissingIndex").unwrap(),
            1 => target.with_multivalues(),
            2 => target.with_collections(),
            _ => target.with_decoded_attachments(),
        };
        let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
        assert_eq!(
            facade
                .scan_projected_progressive("Linked", &[0], &mut |_| panic!("refused target read")),
            Err(QueryError::Unsupported)
        );
    }
}

#[test]
fn unmapped_and_chained_links_do_not_gain_implicit_opening_or_target_query_execution() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes(6)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &[]).unwrap();
    assert_eq!(collect(&facade, &[0]), Err(QueryError::Unsupported));
    assert!(facade.query("Linked").unwrap().is_none());

    let target = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes(6)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("opaque-not-a-host-path"),
            Some("Linked"),
            None,
        )),
        CatalogScope::new(3),
        &pool,
        &|| false,
    )
    .unwrap();
    let mapping = NativeLinkMapping::new(
        "Linked",
        -1,
        "opaque-not-a-host-path",
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
fn late_bad_selected_target_payload_keeps_only_valid_prefix_and_unselected_reads_prune_it() {
    let mut bytes = target_fixture::database();
    let definition =
        target_fixture::definition(&[("Id", 4, 0, 4), ("BadDate", 8, 4, 8)], 3, 11 << 8);
    bytes[ITEMS_TABLE_PAGE * PAGE_BYTES..(ITEMS_TABLE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    let rows = (1..=3_i32)
        .map(|id| {
            let date = if id == 3 { f64::NAN } else { f64::from(id) };
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(date.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[ITEMS_DATA_PAGE * PAGE_BYTES..(ITEMS_DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(ITEMS_TABLE_PAGE as u32, &rows));
    let before = bytes.clone();
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes(6)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget_and_snapshot_limits(
        Cursor::new(bytes.as_slice()),
        CatalogScope::new(2),
        &pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    let mut visits = 0;
    assert_eq!(
        facade.scan_projected_progressive("Linked", &[1], &mut |_| {
            visits += 1;
            Ok(())
        }),
        Err(QueryError::Source)
    );
    assert_eq!(visits, 2);
    assert_eq!(
        collect(&facade, &[0]).unwrap(),
        [1, 2, 3].map(|id| vec![Value::Int32(id)])
    );
    drop(facade);
    drop(target);
    assert_eq!(bytes, before);
}

#[test]
fn late_target_eof_and_expression_errors_do_not_start_the_following_union_arm() {
    for bad_count in [false, true] {
        let mut bytes = target_fixture::database();
        if bad_count {
            support::put32(
                &mut bytes,
                ITEMS_TABLE_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET,
                4,
            );
        }
        let pool = RefCell::new(budget());
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(root_bytes(6)),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget_and_snapshot_limits(
            Cursor::new(bytes),
            CatalogScope::new(2),
            &pool,
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
        let sql = if bad_count {
            "SELECT Id FROM Linked UNION ALL SELECT Score FROM Linked"
        } else {
            "SELECT Id Mod (Id-3) FROM Linked UNION ALL SELECT Score FROM Linked"
        };
        let prepared = plan(&facade, sql);
        let mut rows = Vec::new();
        let result = prepared.execute_progressive(&facade, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        });
        if bad_count {
            assert!(matches!(
                result,
                Err(StreamError::Query(QueryError::Source))
            ));
            assert_eq!(rows, [1, 2, 3].map(|id| vec![Value::Int32(id)]));
        } else {
            assert!(matches!(
                result,
                Err(StreamError::Query(QueryError::Evaluation(
                    EvalError::DivisionByZero
                )))
            ));
            assert_eq!(rows, [1, 0].map(|value| vec![Value::Int64(value)]));
        }
    }
}
