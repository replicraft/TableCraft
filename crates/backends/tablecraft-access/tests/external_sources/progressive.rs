use super::*;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_query::StreamError;

const TABLE_PAGE: usize = 10;
const ROW_COUNT_OFFSET: usize = 16;
const PAGE_SIZE: usize = 4096;

fn source<'a, R: NativeReadSource>(
    input: R,
    scope: u64,
    pool: &'a RefCell<ResourceBudget>,
) -> NativeSource<'a, R> {
    NativeSource::new_with_shared_budget_and_snapshot_limits(
        input,
        CatalogScope::new(scope),
        pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap()
}

fn run(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(parsed.query(), source)?;
    let mut rows = Vec::new();
    prepared.execute_progressive(source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn root_and_authorized_external_pipelines_keep_their_own_values_without_complete_caches() {
    let pool = RefCell::new(allowance());
    let root = source(Cursor::new(base::database()), 1, &pool);
    let forbidden = Cell::new(true);
    let failed = Cell::new(false);
    let target = source(
        Watching {
            inner: Cursor::new(distinct_target_bytes()),
            forbid_rows: &forbidden,
            fail_rows: &failed,
        },
        2,
        &pool,
    );
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let sql = "SELECT Q.Id,Q.Score FROM (SELECT Id,Score FROM Items IN \"remote\") Q";
    let parsed = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &facade).unwrap();
    forbidden.set(false);
    let mut rows = Vec::new();
    prepared
        .execute_progressive(&facade, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(
        rows,
        (1..=3)
            .map(|id| vec![Value::Int32(id), Value::Int32(id * 111)])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        run(
            &facade,
            "SELECT Q.Id,Q.Score FROM (SELECT Id,Score FROM Items) Q"
        )
        .unwrap(),
        (1..=3)
            .map(|id| vec![Value::Int32(id), Value::Int32(id * 10)])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        target.scan("Items", &mut |_| Ok(())),
        Err(QueryError::StructureLimit)
    );
}

#[test]
fn progressive_facades_preserve_visitor_errors_and_canonical_local_names() {
    let pool = RefCell::new(allowance());
    let root = source(Cursor::new(base::database()), 1, &pool);
    let target = source(Cursor::new(distinct_target_bytes()), 2, &pool);
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let external = facade.external_source(request("remote")).unwrap().unwrap();
    for provider in [&facade as &dyn Source, external.source()] {
        let mut visits = 0;
        assert_eq!(
            provider.scan_projected_progressive("items", &[0], &mut |_| {
                visits += 1;
                Err(QueryError::Width)
            }),
            Err(QueryError::Width)
        );
        assert_eq!(visits, 1);
    }
}

#[test]
fn failed_external_eof_prevents_aggregate_publication() {
    let pool = RefCell::new(allowance());
    let root = source(Cursor::new(base::database()), 1, &pool);
    let mut bytes = distinct_target_bytes();
    support::put32(&mut bytes, TABLE_PAGE * PAGE_SIZE + ROW_COUNT_OFFSET, 4);
    let target = source(Cursor::new(bytes), 2, &pool);
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let parsed = parse_query(
        "SELECT Count(*) FROM Items IN \"remote\"",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &facade).unwrap();
    let mut visits = 0;
    assert!(
        prepared
            .execute_progressive(&facade, &[], Default::default(), &mut |_| {
                visits += 1;
                Ok::<(), ()>(())
            })
            .is_err()
    );
    assert_eq!(visits, 0);
}

#[test]
fn external_progressive_callbacks_never_follow_stored_link_paths() {
    let pool = RefCell::new(allowance());
    let root = source(Cursor::new(base::database()), 1, &pool);
    let target = source(
        Cursor::new(linked_fixture::database(
            6,
            Some("/stored/path/must/not/open.accdb"),
            Some("Items"),
            None,
        )),
        2,
        &pool,
    );
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let external = facade.external_source(request("remote")).unwrap().unwrap();
    let mut visits = 0;
    assert!(
        external
            .source()
            .scan_projected_progressive("Linked", &[0], &mut |_| {
                visits += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(visits, 0);
}
