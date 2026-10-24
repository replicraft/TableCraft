#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "../../../engines/tablecraft-ace/tests/narrow_indexes/fixture.rs"]
mod narrow_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
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
use tablecraft_query::{QueryError, QueryResult, Source, prepare};
use tablecraft_sql::parse_query;

struct Reads {
    cursor: Cursor<Vec<u8>>,
    index_reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Reads {}

impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor.position() == 14 * 4096 {
            self.index_reads.set(self.index_reads.get() + 1);
        }
        self.cursor.read(bytes)
    }
}

impl Seek for Reads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

fn bytes(code: u8, small: bool, descending: bool) -> Vec<u8> {
    let mut bytes = narrow_fixture::database(code, small, descending, &[None, Some(0), Some(1)], 0);
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn run(source: &dyn Source, sql: &str) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source)?.execute(source, &[], Default::default())
}

#[test]
fn automatic_narrow_selections_are_lazy_and_do_not_publish_partial_caches() {
    for code in [1, 2, 5, 6] {
        for small in [false, true] {
            for descending in [false, true] {
                let reads = Rc::new(Cell::new(0));
                let source = NativeSource::new_with_snapshot_limits(
                    Reads {
                        cursor: Cursor::new(bytes(code, small, descending)),
                        index_reads: reads.clone(),
                    },
                    CatalogScope::new(1),
                    budget(),
                    &|| false,
                    TableSnapshotLimits::new(1, 1).unwrap(),
                )
                .unwrap()
                .with_auto_indexes();

                for sql in [
                    "SELECT Id FROM Items WHERE Score=0",
                    "SELECT * FROM Items WHERE Score=1",
                    "SELECT Id FROM Items WHERE Score IS NULL",
                ] {
                    let query = parse_query(sql, Default::default()).unwrap();
                    let before = reads.get();
                    let plan = prepare(query.query(), &source).unwrap();
                    assert_eq!(reads.get(), before, "preparation must not traverse indexes");
                    let result = plan.execute(&source, &[], Default::default()).unwrap();
                    assert_eq!(result.rows().len(), 1);
                    assert!(reads.get() > before);
                }
                assert_eq!(
                    run(&source, "SELECT Count(*) FROM Items").err(),
                    Some(QueryError::StructureLimit)
                );
            }
        }
    }
}

#[test]
fn unsuitable_or_unselected_indexes_are_not_traversed() {
    for flags in [0x80, 0x82, 0x84] {
        let mut data = bytes(5, true, false);
        data[10 * 4096 + 189] = flags;
        data[14 * 4096] = 0;
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Reads {
                cursor: Cursor::new(data),
                index_reads: reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_auto_indexes();

        // The index is on Score. Ignore-NULL and unknown flags also disqualify
        // it for matching Score predicates before any traversal starts.
        let filter = if flags == 0x80 { "Id=2" } else { "Score=0" };
        assert_eq!(
            run(&source, &format!("SELECT Id FROM Items WHERE {filter}"))
                .unwrap()
                .rows(),
            [vec![Value::Int32(2)]]
        );
        assert_eq!(reads.get(), 0);
    }
}

#[test]
fn corrupt_chosen_indexes_fail_without_sequential_retry() {
    let mut data = bytes(5, true, false);
    data[14 * 4096] = 0;
    let source = NativeSource::new(Cursor::new(data), CatalogScope::new(1), budget(), &|| false)
        .unwrap()
        .with_auto_indexes();
    assert_eq!(
        run(&source, "SELECT Id FROM Items WHERE Score=0").err(),
        Some(QueryError::Source)
    );

    // A later unconstrained scan is independent and can still read all rows.
    assert_eq!(
        run(&source, "SELECT Count(*) FROM Items").unwrap().rows(),
        [vec![Value::Int64(3)]]
    );
}

#[test]
fn automatic_mode_keeps_explicit_indexes_and_whole_predicate_safety() {
    let source = NativeSource::new(
        Cursor::new(bytes(5, true, false)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_auto_indexes()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        run(&source, "SELECT Id FROM Items WHERE Score=0").err(),
        Some(QueryError::UnknownName)
    );

    let source = NativeSource::new(
        Cursor::new(bytes(5, true, false)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_auto_indexes();
    assert_eq!(
        run(&source, "SELECT Id FROM Items WHERE Score>10 AND 1/0=0").err(),
        Some(tablecraft_expressions::EvalError::DivisionByZero.into())
    );
}

fn cancel_attempt(stop: Option<usize>) -> (Result<QueryResult, QueryError>, usize) {
    let active = Cell::new(false);
    let checks = Cell::new(0);
    let cancelled = || {
        if !active.get() {
            return false;
        }
        checks.set(checks.get() + 1);
        stop.is_some_and(|stop| checks.get() >= stop)
    };
    let source = NativeSource::new(
        Cursor::new(bytes(5, true, false)),
        CatalogScope::new(1),
        budget(),
        &cancelled,
    )
    .unwrap()
    .with_auto_indexes();
    let query = parse_query("SELECT Id FROM Items WHERE Score=0", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    active.set(true);
    let result = plan.execute(&source, &[], Default::default());
    (result, checks.get())
}

#[test]
fn every_observed_automatic_selection_checkpoint_can_cancel() {
    let (result, observed) = cancel_attempt(None);
    assert_eq!(result.unwrap().rows(), [vec![Value::Int32(2)]]);
    assert!(observed > 0);

    for stop in 1..=observed {
        assert_eq!(
            cancel_attempt(Some(stop)).0.err(),
            Some(QueryError::Cancelled)
        );
    }
}

#[test]
fn automatic_paths_preserve_table_order_for_top_and_encounter_aggregates() {
    for code in [1, 2, 5, 6] {
        for small in [false, true] {
            for descending in [false, true] {
                for (sql, expected) in [
                    (
                        "SELECT Id FROM Items WHERE Score>=0",
                        vec![vec![Value::Int32(2)], vec![Value::Int32(3)]],
                    ),
                    (
                        "SELECT TOP 1 Id FROM Items WHERE Score>=0",
                        vec![vec![Value::Int32(2)]],
                    ),
                    (
                        "SELECT First(Id),Last(Id) FROM Items WHERE Score>=0",
                        vec![vec![Value::Int32(2), Value::Int32(3)]],
                    ),
                ] {
                    let source = NativeSource::new(
                        Cursor::new(bytes(code, small, descending)),
                        CatalogScope::new(1),
                        budget(),
                        &|| false,
                    )
                    .unwrap()
                    .with_auto_indexes();
                    assert_eq!(run(&source, sql).unwrap().rows(), expected);
                }
            }
        }
    }
}
