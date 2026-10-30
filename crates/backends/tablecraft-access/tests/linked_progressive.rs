#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as target_fixture;

#[path = "linked_progressive/boundaries.rs"]
mod boundaries;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{LinkedNativeSource, NativeLinkMapping, NativeSource};
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, PreparedQuery, QueryError, Source, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const FORMAT_OFFSET: usize = 20;
const HEADER_MAGIC_START: usize = 4;
const ITEMS_TABLE_PAGE: usize = 10;
const ITEMS_DATA_PAGE: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

fn profile(mut bytes: Vec<u8>, code: u8) -> Vec<u8> {
    bytes[FORMAT_OFFSET] = code;
    let magic = if code == 1 {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    };
    bytes[HEADER_MAGIC_START..FORMAT_OFFSET].copy_from_slice(magic);
    bytes
}

fn root_bytes(code: u8) -> Vec<u8> {
    profile(
        linked_fixture::database(6, Some("opaque-not-a-host-path"), Some("items"), None),
        code,
    )
}

fn mappings<'a>(target: &'a NativeSource<'a, impl NativeReadSource>) -> [NativeLinkMapping<'a>; 1] {
    [NativeLinkMapping::new(
        "Linked",
        -1,
        "opaque-not-a-host-path",
        target.handle(),
        "items",
        10,
    )]
}

struct Watching<'a> {
    input: Cursor<&'a [u8]>,
    forbid_rows: &'a Cell<bool>,
    fail_rows: &'a Cell<bool>,
    row_reads: &'a Cell<usize>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.input.position() as usize / PAGE_BYTES == ITEMS_DATA_PAGE {
            assert!(!self.forbid_rows.get(), "binding read target records");
            self.row_reads.set(self.row_reads.get() + 1);
            if self.fail_rows.get() {
                return Err(std::io::Error::other("injected target read failure"));
            }
        }
        self.input.read(bytes)
    }
}

impl Seek for Watching<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}

fn plan(source: &dyn Source, sql: &str) -> PreparedQuery {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source).unwrap()
}

fn collect(source: &impl Source, indices: &[usize]) -> Result<Vec<Vec<Value>>, QueryError> {
    let mut rows = Vec::new();
    source.scan_projected_progressive("Linked", indices, &mut |row| {
        rows.push(
            (0..row.len())
                .map(|index| row.get(index).unwrap().clone())
                .collect(),
        );
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn authorized_link_streams_without_binding_rows_or_promoting_a_complete_cache() {
    for code in [1, 2, 5, 6] {
        for batch in [1, 2, 5] {
            let root_bytes = root_bytes(code);
            let target_bytes = profile(target_fixture::database(), code);
            let root_before = root_bytes.clone();
            let target_before = target_bytes.clone();
            let pool = RefCell::new(budget());
            let root = NativeSource::new_with_shared_budget(
                Cursor::new(root_bytes.as_slice()),
                CatalogScope::new(1),
                &pool,
                &|| false,
            )
            .unwrap();
            let forbid_rows = Cell::new(true);
            let fail_rows = Cell::new(false);
            let row_reads = Cell::new(0);
            let target = NativeSource::new_with_shared_budget_and_snapshot_limits(
                Watching {
                    input: Cursor::new(target_bytes.as_slice()),
                    forbid_rows: &forbid_rows,
                    fail_rows: &fail_rows,
                    row_reads: &row_reads,
                },
                CatalogScope::new(2),
                &pool,
                &|| false,
                TableSnapshotLimits::new(1, batch).unwrap(),
            )
            .unwrap();
            let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
            let prepared = plan(
                &facade,
                "SELECT Id FROM Linked UNION ALL SELECT Id FROM linked",
            );
            assert_eq!(row_reads.get(), 0);
            forbid_rows.set(false);
            let mut rows = Vec::new();
            let summary = prepared
                .execute_progressive(&facade, &[], ExecutionOptions::default(), &mut |row| {
                    pool.try_borrow_mut()
                        .unwrap()
                        .charge(Resource::Steps, 1)
                        .unwrap();
                    assert_eq!(collect(&facade, &[0]), Err(QueryError::Source));
                    rows.push(row.to_vec());
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(summary.scanned_rows, 6);
            assert_eq!(summary.emitted_rows, 6);
            assert_eq!(rows, [1, 2, 3, 1, 2, 3].map(|id| vec![Value::Int32(id)]));
            assert_eq!(
                facade.scan_projected("Linked", &[0], None, &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
            assert_eq!(collect(&facade, &[]).unwrap(), vec![vec![]; 3]);

            fail_rows.set(true);
            assert_eq!(collect(&facade, &[0]), Err(QueryError::Source));
            drop(facade);
            drop(target);
            drop(root);
            assert_eq!(root_bytes, root_before);
            assert_eq!(target_bytes, target_before);
        }
    }
}

#[test]
fn failed_visitor_releases_target_cursor_and_retry_starts_at_first_record() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes(6)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget_and_snapshot_limits(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    let mut visits = 0;
    assert_eq!(
        facade.scan_projected_progressive("Linked", &[0], &mut |_| {
            visits += 1;
            Err(QueryError::Parameter)
        }),
        Err(QueryError::Parameter)
    );
    assert_eq!(visits, 1);
    assert_eq!(
        collect(&facade, &[0]).unwrap(),
        [1, 2, 3].map(|id| vec![Value::Int32(id)])
    );
}

#[test]
fn facade_without_mappings_keeps_root_local_progressive_reads() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget_and_snapshot_limits(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &[]).unwrap();
    let prepared = plan(&facade, "SELECT Score FROM Items");
    let mut rows = Vec::new();
    prepared
        .execute_progressive(&facade, &[], ExecutionOptions::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(rows, [10, 20, 30].map(|score| vec![Value::Int32(score)]));
    assert_eq!(
        facade.scan("Items", &mut |_| Ok(())),
        Err(QueryError::StructureLimit)
    );
}
