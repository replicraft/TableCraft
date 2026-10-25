use super::*;
use std::cell::RefCell;
use tablecraft_core::limits::Resource;

const TABLE_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;

#[test]
fn cached_metadata_and_schema_agreement_are_checked_before_user_rows() {
    let bytes = scalar_fixture::database(6, 10, 39, false, &[Some(b"x\0".to_vec()), None]);
    let watched = Watching::new(bytes);
    let forbidden = watched.forbidden.clone();
    let changed = watched.changed_capacity.clone();
    let reads = watched.data_reads.clone();
    forbidden.set(true);
    let source = source(watched, 1, &|| false, true);
    assert_eq!(source.columns("Items").unwrap().len(), 2);
    assert_eq!(
        source.is_stored_calculation("Items", 1).unwrap(),
        Some(true)
    );
    assert_eq!(reads.get(), 0);

    changed.set(true);
    assert_eq!(collect(&source, &[1]), Err(QueryError::SchemaChanged));
    assert_eq!(reads.get(), 0);
    changed.set(false);
    forbidden.set(false);
    assert_eq!(
        collect(&source, &[1]).unwrap(),
        [vec![Value::Text("x".into())], vec![Value::Null]]
    );
    assert!(reads.get() > 0);
}

#[test]
fn cached_stream_eof_counts_remain_checked_without_promoting_failed_prefixes() {
    let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
    support::put32(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET,
        4,
    );
    let source = source(Cursor::new(bytes), 1, &|| false, true);
    let mut rows = Vec::new();
    let result = source.scan_projected_progressive("Items", &[1], &mut |row| {
        rows.push(row.get(0).unwrap().clone());
        Ok(())
    });
    assert_eq!(result, Err(QueryError::Source));
    assert_eq!(rows, [Value::Int32(999), Value::Null, Value::Int32(41)]);
    assert_eq!(collect(&source, &[1]), Err(QueryError::Source));
    assert!(buffered(&source, &[1]).is_err());
}

#[test]
fn cached_batches_share_native_limits_and_release_budget_before_visitors() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let pool = RefCell::new(budget());
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            Cursor::new(calculated_fixture::database(&[Some(999), None, Some(41)])),
            SCOPE,
            &pool,
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap()
        .with_stored_calculations()
        .unwrap();
        let mut seen = Vec::new();
        let failure = source.scan_projected_progressive("Items", &[1], &mut |row| {
            seen.push(row.get(0).unwrap().clone());
            assert_eq!(collect(&source, &[1]), Err(QueryError::Source));
            let mut available = pool.try_borrow_mut().unwrap();
            let remaining = available.remaining(resource);
            available.charge(resource, remaining).unwrap();
            Ok(())
        });
        assert!(matches!(failure, Err(QueryError::Limit(_))));
        assert_eq!(seen, [Value::Int32(999)]);
        assert_eq!(pool.borrow().remaining(resource), 0);
    }
}
