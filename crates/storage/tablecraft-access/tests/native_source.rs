use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
fn copy() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
struct CountingReader {
    cursor: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
}
impl Read for CountingReader {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.cursor.read(b)
    }
}
impl Seek for CountingReader {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(p)
    }
}
#[test]
fn actual_query_consumer_caches_tables_and_stops_when_visitor_rejects() {
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        CountingReader {
            cursor: Cursor::new(copy()),
            reads: reads.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.columns("ROULETTE_TB").unwrap().len(), 3);
    let initial = reads.get();
    let mut count = 0;
    assert_eq!(
        source.scan("roulette_tb", &mut |_| {
            count += 1;
            Err(QueryError::Cancelled)
        }),
        Err(QueryError::Cancelled)
    );
    assert_eq!(count, 1);
    assert_eq!(reads.get(), initial);
    let parsed = tablecraft_sql::parse_query(
        "SELECT COLOR_ID, Count(*) AS N FROM ROULETTE_TB GROUP BY COLOR_ID ORDER BY COLOR_ID",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result
            .rows()
            .iter()
            .map(|r| match r[1] {
                Value::Int64(n) => n,
                _ => panic!("count carrier"),
            })
            .sum::<i64>(),
        38
    );
    assert_eq!(reads.get(), initial);
}
#[test]
fn unsupported_and_missing_sources_fail_and_unused_unsupported_tables_do_not_block() {
    let source = NativeSource::new(Cursor::new(copy()), CatalogScope::new(7), budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        source.columns("private-missing-name").err(),
        Some(QueryError::UnknownName)
    );
    assert_eq!(
        source.columns("CMD_LINE_TB").err(),
        Some(QueryError::Unsupported)
    );
    assert!(source.columns("ROULETTE_TB").is_ok());
    assert_eq!(source.columns("tblContacts").unwrap().len(), 7);
    let mut count = 0;
    source
        .scan("tblContacts", &mut |_| {
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn cancellation_also_stops_cached_scans_and_constructor_limits_apply() {
    let cancel = Cell::new(false);
    let callback = || cancel.get();
    let source = NativeSource::new(
        Cursor::new(copy()),
        CatalogScope::new(7),
        budget(),
        &callback,
    )
    .unwrap();
    source.columns("ROULETTE_TB").unwrap();
    cancel.set(true);
    assert_eq!(
        source.scan("ROULETTE_TB", &mut |_| panic!("cancelled visitor")),
        Err(QueryError::Cancelled)
    );
    assert!(
        NativeSource::new(
            Cursor::new(copy()),
            CatalogScope::new(7),
            ResourceBudget::new(66, 0, 10),
            &|| false
        )
        .is_err()
    );
}
