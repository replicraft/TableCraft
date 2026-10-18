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
        source.columns("MSysResources").err(),
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

struct SwitchReader {
    cursor: Cursor<Vec<u8>>,
    fail: Rc<Cell<bool>>,
    reads: Rc<Cell<usize>>,
}
impl Read for SwitchReader {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        if self.fail.get() {
            return Err(std::io::Error::other("private payload"));
        }
        self.cursor.read(bytes)
    }
}
impl Seek for SwitchReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}
#[test]
fn failed_load_publishes_no_rows_and_explicit_retry_only_caches_a_complete_table() {
    let fail = Rc::new(Cell::new(false));
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        SwitchReader {
            cursor: Cursor::new(copy()),
            fail: fail.clone(),
            reads: reads.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap();
    fail.set(true);
    assert_eq!(
        source.scan("ROULETTE_TB", &mut |_| panic!("failed load exposed a row")),
        Err(QueryError::Source)
    );
    fail.set(false);
    assert_eq!(source.columns("ROULETTE_TB").unwrap().len(), 3);
    let loaded = reads.get();
    fail.set(true);
    let mut count = 0;
    source
        .scan("ROULETTE_TB", &mut |_| {
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 38);
    assert_eq!(reads.get(), loaded);
}
#[test]
fn repeated_failed_loads_exhaust_the_original_shared_allowance() {
    let fail = Rc::new(Cell::new(false));
    let source = NativeSource::new(
        SwitchReader {
            cursor: Cursor::new(copy()),
            fail: fail.clone(),
            reads: Rc::new(Cell::new(0)),
        },
        CatalogScope::new(7),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 10_000),
        &|| false,
    )
    .unwrap();
    fail.set(true);
    for _ in 0..=10_000 {
        match source.scan("ROULETTE_TB", &mut |_| panic!("failed load exposed a row")) {
            Err(QueryError::Source) => {}
            Err(QueryError::Limit(_)) => return,
            _ => panic!("unexpected failed-load result"),
        }
    }
    panic!("failed work incorrectly reset the allowance");
}

#[test]
fn explicit_index_snapshots_are_cached_and_invalid_choices_never_fall_back() {
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
    .unwrap()
    .with_index("ROULETTE_TB", "ORDR_NBR")
    .unwrap();
    source.columns("ROULETTE_TB").unwrap();
    let loaded = reads.get();
    let mut ids = Vec::new();
    source
        .scan("ROULETTE_TB", &mut |row| {
            ids.push(row[0].clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(ids, (1..=38).map(Value::Int32).collect::<Vec<_>>());
    assert_eq!(reads.get(), loaded);
    let source = source.with_index("ROULETTE_TB", "Number").unwrap();
    assert_eq!(
        source.columns("ROULETTE_TB").err(),
        Some(QueryError::Unsupported)
    );
}
