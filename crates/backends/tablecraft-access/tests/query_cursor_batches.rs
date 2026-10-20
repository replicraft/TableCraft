#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/large_maps/fixture.rs"]
mod map_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const PAGE_BYTES: u64 = 4096;
const LATE_DATA_PAGE: u64 = map_fixture::FIRST_DATA_PAGE as u64 + 5000;
const SCOPE: CatalogScope = CatalogScope::new(1);

struct Controlled {
    source: Cursor<Vec<u8>>,
    fail: Rc<Cell<bool>>,
    reads: Rc<Cell<u64>>,
}
impl tablecraft_ace::NativeReadSource for Controlled {}
impl Read for Controlled {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        if self.fail.get() && self.source.position() == LATE_DATA_PAGE * PAGE_BYTES {
            return Err(std::io::Error::other("private diagnostic"));
        }
        self.source.read(bytes)
    }
}
impl Seek for Controlled {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(position)
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 30_000, 1_000_000)
}
fn input(code: u8, mixed: bool) -> Controlled {
    let mut bytes = map_fixture::database(6000, true, false);
    bytes[20] = code;
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    if mixed {
        bytes[10 * PAGE_BYTES as usize + 63 + 25] = 0xff;
    }
    Controlled {
        source: Cursor::new(bytes),
        fail: Rc::new(Cell::new(false)),
        reads: Rc::new(Cell::new(0)),
    }
}
fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn large_projected_queries_and_full_scans_reuse_complete_caches_in_every_profile() {
    for code in [1, 2, 5, 6] {
        for mixed in [false, true] {
            let input = input(code, mixed);
            let reads = input.reads.clone();
            let source = NativeSource::new(input, SCOPE, budget(), &|| false).unwrap();
            assert_eq!(
                execute(&source, "SELECT TOP 1 Id FROM Items ORDER BY Id DESC").unwrap(),
                [vec![Value::Int32(6000)]]
            );
            let loaded = reads.get();
            assert_eq!(
                execute(&source, "SELECT TOP 1 Id FROM Items ORDER BY Id").unwrap(),
                [vec![Value::Int32(1)]]
            );
            assert_eq!(reads.get(), loaded);
            let mut count = 0;
            source
                .scan("Items", &mut |row| {
                    count += 1;
                    assert_eq!(row[0], Value::Int32(count));
                    assert_eq!(row.len(), if mixed { 1 } else { 2 });
                    Ok(())
                })
                .unwrap();
            assert_eq!(count, 6000);
            let loaded = reads.get();
            source
                .scan("Items", &mut |_| Err(QueryError::Unsupported))
                .unwrap_err();
            assert_eq!(reads.get(), loaded);
        }
    }
}

#[test]
fn late_failed_materialization_publishes_no_rows_or_cache_and_explicit_retry_is_complete() {
    let input = input(6, false);
    let fail = input.fail.clone();
    let reads = input.reads.clone();
    let source = NativeSource::new(input, SCOPE, budget(), &|| false).unwrap();
    fail.set(true);
    assert_eq!(
        source.scan("Items", &mut |_| panic!("partial table escaped")),
        Err(QueryError::Source)
    );
    fail.set(false);
    let mut count = 0;
    source
        .scan("Items", &mut |_| {
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 6000);
    let loaded = reads.get();
    fail.set(true);
    source.scan("Items", &mut |_| Ok(())).unwrap();
    assert_eq!(reads.get(), loaded);
}

#[test]
fn batches_never_refill_the_original_byte_or_row_budget() {
    for controls in [
        ResourceBudget::new(16 * 1024 * 1024, 30_000, 1_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 500, 1_000_000),
    ] {
        let source = NativeSource::new(input(6, false), SCOPE, controls, &|| false).unwrap();
        assert!(matches!(
            source.scan("Items", &mut |_| panic!("partial table escaped")),
            Err(QueryError::Limit(_))
        ));
    }
}
