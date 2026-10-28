#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ignored_null_indexes/fixture.rs"]
mod ignored;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, QueryResult, Source, prepare};
use tablecraft_sql::parse_query;

const VALUES: &[Option<i32>] = &[Some(20), None, Some(10), Some(30), None, Some(20)];

struct Reads {
    cursor: Cursor<Vec<u8>>,
    index_reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Reads {}

impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor.position() == (ignored::ROOT * ignored::PAGE_BYTES) as u64 {
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
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 2_000_000)
}

fn run(source: &dyn Source, sql: &str) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, &[], Default::default())
}

fn ids(values: &[i32]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|value| vec![Value::Int32(*value)])
        .collect()
}

#[test]
fn automatic_non_null_selections_keep_table_order_and_required_indexes_keep_key_order() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = ignored::database(code, descending, false, true, VALUES);
            let source = NativeSource::new(
                Cursor::new(bytes.clone()),
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap()
            .with_auto_indexes();
            assert_eq!(
                run(&source, "SELECT Id FROM Items WHERE Score>=10")
                    .unwrap()
                    .rows(),
                ids(&[1, 3, 4, 6])
            );

            let required =
                NativeSource::new(Cursor::new(bytes), CatalogScope::new(2), budget(), &|| {
                    false
                })
                .unwrap()
                .with_index("Items", "ScoreAlias")
                .unwrap();
            let expected = if descending {
                &[4, 1, 6, 3]
            } else {
                &[3, 1, 6, 4]
            };
            assert_eq!(
                run(&required, "SELECT Id FROM Items WHERE Score>=10")
                    .unwrap()
                    .rows(),
                ids(expected)
            );
            for sql in [
                "SELECT Id FROM Items",
                "SELECT Id FROM Items WHERE Score IS NULL",
            ] {
                assert_eq!(run(&required, sql).err(), Some(QueryError::Unsupported));
            }
        }
    }
}

#[test]
fn omitted_nulls_and_partial_reads_never_become_a_complete_table_cache() {
    for code in [1, 2, 5, 6] {
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new_with_snapshot_limits(
            Reads {
                cursor: Cursor::new(ignored::database(code, false, false, false, VALUES)),
                index_reads: reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
            TableSnapshotLimits::new(2, 1).unwrap(),
        )
        .unwrap()
        .with_auto_indexes();
        let query = parse_query("SELECT Id FROM Items WHERE Score=20", Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(
            reads.get(),
            0,
            "binding must not traverse the selected index"
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            ids(&[1, 6])
        );
        assert!(reads.get() > 0);
        let before = reads.get();
        for sql in [
            "SELECT Count(*) FROM Items",
            "SELECT Id FROM Items WHERE Score IS NULL",
        ] {
            assert_eq!(run(&source, sql).err(), Some(QueryError::StructureLimit));
        }
        assert_eq!(
            reads.get(),
            before,
            "NULL and complete scans must decline the omitting index"
        );
    }
}

#[test]
fn null_predicates_decline_ignoring_indexes_and_corrupt_chosen_indexes_never_fallback() {
    for code in [1, 2, 5, 6] {
        let mut bytes = ignored::database(code, false, false, false, VALUES);
        bytes[ignored::ROOT * ignored::PAGE_BYTES] = 0;
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Reads {
                cursor: Cursor::new(bytes),
                index_reads: reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_auto_indexes();
        assert_eq!(
            run(&source, "SELECT Id FROM Items WHERE Score IS NULL")
                .unwrap()
                .rows(),
            ids(&[2, 5])
        );
        assert_eq!(reads.get(), 0);
        // A complete cached table is a legitimate later source. Use a fresh source
        // to prove that selecting a corrupt index itself cannot retry sequentially.
        let mut bytes = ignored::database(code, false, false, false, VALUES);
        bytes[ignored::ROOT * ignored::PAGE_BYTES] = 0;
        let selected =
            NativeSource::new(Cursor::new(bytes), CatalogScope::new(2), budget(), &|| {
                false
            })
            .unwrap()
            .with_auto_indexes();
        assert_eq!(
            run(&selected, "SELECT Id FROM Items WHERE Score=20").err(),
            Some(QueryError::Source)
        );
        assert_eq!(
            run(&selected, "SELECT Count(*) FROM Items").unwrap().rows(),
            [vec![Value::Int64(6)]]
        );
    }
}

#[test]
fn automatic_ignore_null_hints_retain_whole_predicate_eager_errors() {
    let source = NativeSource::new(
        Cursor::new(ignored::database(5, false, false, false, VALUES)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_auto_indexes();
    assert_eq!(
        run(&source, "SELECT Id FROM Items WHERE Score=99 AND 1/0=0").err(),
        Some(EvalError::DivisionByZero.into())
    );
}
