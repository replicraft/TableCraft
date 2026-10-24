#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "cached_calculation_queries/hyperlink.rs"]
mod hyperlink;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_memos/fixture.rs"]
mod memo_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

struct Watching {
    data: Cursor<Vec<u8>>,
    forbid: Rc<Cell<bool>>,
    reads: Rc<Cell<usize>>,
}
impl NativeReadSource for Watching {}
impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.data.position() as usize / 4096 == 12 {
            assert!(
                !self.forbid.get(),
                "metadata binding or cache reuse read user rows"
            );
            self.reads.set(self.reads.get() + 1);
        }
        self.data.read(bytes)
    }
}
impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(position)
    }
}

#[test]
fn cached_queries_are_explicit_metadata_only_and_reuse_complete_stored_values() {
    for code in [5, 6] {
        let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
        bytes[20] = code;
        let source = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        assert!(execute(&source, "SELECT Score FROM Items").is_err());
        let forbidden = Rc::new(Cell::new(true));
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new_with_snapshot_limits(
            Watching {
                data: Cursor::new(bytes),
                forbid: forbidden.clone(),
                reads: reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
            TableSnapshotLimits::new(100, 1).unwrap(),
        )
        .unwrap()
        .with_stored_calculations()
        .unwrap();
        let parsed = parse_query("SELECT Id,Score FROM Items", ParseLimits::default()).unwrap();
        let prepared = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            source.is_stored_calculation("Items", 0).unwrap(),
            Some(false)
        );
        assert_eq!(
            source.is_stored_calculation("Items", 1).unwrap(),
            Some(true)
        );
        assert_eq!(reads.get(), 0);
        forbidden.set(false);
        assert_eq!(
            prepared
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [
                vec![Value::Int32(1), Value::Int32(999)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Int32(41)],
            ]
        );
        assert!(reads.get() > 0);
        forbidden.set(true);
        assert_eq!(
            execute(&source, "SELECT Sum(Score) FROM Items").unwrap(),
            [vec![Value::Int64(1040)]]
        );
        assert_eq!(
            execute(&source, "SELECT Id FROM Items WHERE Score>100 ORDER BY Id").unwrap(),
            [vec![Value::Int32(1)]]
        );
        assert!(source.with_stored_calculations().is_err());
    }
}

#[test]
fn ordinary_projection_prunes_corrupt_caches_and_late_failures_never_become_complete() {
    let mut bytes = calculated_fixture::database(&[Some(20), Some(41)]);
    let second =
        u16::from_le_bytes(bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    bytes[12 * 4096 + second + 6] = 1;
    let source = NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        CatalogScope::new(1),
        budget(),
        &|| false,
        TableSnapshotLimits::new(100, 1).unwrap(),
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Id FROM Items").unwrap(),
        [vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    for sql in [
        "SELECT Score FROM Items",
        "SELECT TOP 1 Score FROM Items",
        "SELECT Score FROM Items WHERE Id=999",
        "SELECT Count(Score) FROM Items",
    ] {
        assert!(execute(&source, sql).is_err(), "{sql}");
    }
    assert_eq!(execute(&source, "SELECT Id FROM Items").unwrap().len(), 2);
}

#[test]
fn required_indexes_refuse_cached_reads_but_auto_indexes_do_not_change_results() {
    let bytes = calculated_fixture::database(&[Some(999), Some(41)]);
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score FROM Items").err(),
        Some(QueryError::Unsupported)
    );
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_auto_indexes();
    assert_eq!(
        execute(&source, "SELECT Score FROM Items WHERE Score=999").unwrap(),
        [vec![Value::Int32(999)]]
    );
}

#[test]
fn unsupported_calculated_declarations_stay_visible_without_blocking_ordinary_reads() {
    let mut bytes = calculated_fixture::database(&[Some(20)]);
    bytes[10 * 4096 + 63 + 25 + 23] = 38;
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    assert_eq!(
        source.unavailable_columns("Items").unwrap()[0].as_str(),
        "Score"
    );
    assert_eq!(
        execute(&source, "SELECT Id FROM Items").unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert!(execute(&source, "SELECT Score FROM Items").is_err());
    assert!(execute(&source, "SELECT * FROM Items").is_err());
}

#[test]
fn source_limits_and_cancellation_remain_explicit() {
    let bytes = calculated_fixture::database(&[Some(20), Some(41)]);
    let armed = Cell::new(false);
    let cancelled = || armed.get();
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &cancelled,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    source.columns("Items").unwrap();
    armed.set(true);
    assert_eq!(
        execute(&source, "SELECT Score FROM Items").err(),
        Some(QueryError::Cancelled)
    );
    armed.set(false);
    assert_eq!(
        execute(&source, "SELECT Score FROM Items").unwrap().len(),
        2
    );
    let source = NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        CatalogScope::new(1),
        budget(),
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    assert!(execute(&source, "SELECT TOP 1 Score FROM Items").is_err());
    assert_eq!(
        source.is_stored_calculation("Items", usize::MAX).unwrap(),
        None
    );
}
