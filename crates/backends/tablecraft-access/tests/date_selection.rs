#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, QueryResult, Source, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const INDEX_PAGE: usize = 14;

struct Watched {
    input: Cursor<Vec<u8>>,
    index_reads: Rc<Cell<usize>>,
    data_reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Watched {}

impl Read for Watched {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self.input.position() as usize {
            position if position == INDEX_PAGE * PAGE_BYTES => {
                self.index_reads.set(self.index_reads.get() + 1);
            }
            position if position == DATA_PAGE * PAGE_BYTES => {
                self.data_reads.set(self.data_reads.get() + 1);
            }
            _ => {}
        }
        self.input.read(bytes)
    }
}

impl Seek for Watched {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

fn date(serial: f64) -> LegacyDateTime {
    LegacyDateTime::from_ole_days(serial).unwrap()
}

fn database(profile: u8, descending: bool, values: &[Option<f64>]) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            vec![
                Some(index as i64 + 1),
                value.map(|value| value.to_bits() as i64),
            ]
        })
        .collect::<Vec<_>>();
    let mut bytes = fixture::database(profile, &[4, 8], &[(1, descending)], &rows, 0);
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn native(bytes: Vec<u8>, maximum: u32) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        CatalogScope::new(1),
        budget(),
        &|| false,
        TableSnapshotLimits::new(maximum, 2).unwrap(),
    )
    .unwrap()
}

fn run(source: &dyn Source, sql: &str) -> Result<QueryResult, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)?.execute(source, &[], Default::default())
}

#[test]
fn automatic_date_points_remain_metadata_only_until_execution_and_do_not_cache_prefixes() {
    for profile in [1, 2, 5, 6] {
        for descending in [false, true] {
            let index_reads = Rc::new(Cell::new(0));
            let data_reads = Rc::new(Cell::new(0));
            let source = NativeSource::new_with_snapshot_limits(
                Watched {
                    input: Cursor::new(database(
                        profile,
                        descending,
                        &[None, Some(-1.25), Some(1.25)],
                    )),
                    index_reads: index_reads.clone(),
                    data_reads: data_reads.clone(),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, 1).unwrap(),
            )
            .unwrap()
            .with_auto_indexes();
            let syntax = parse_query(
                "PARAMETERS P DATETIME; SELECT Id FROM Items WHERE Score=P",
                Default::default(),
            )
            .unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            assert_eq!(index_reads.get(), 0);
            assert_eq!(data_reads.get(), 0);

            for (serial, expected) in [(-1.25, 2), (1.25, 3)] {
                let result = plan
                    .execute(
                        &source,
                        &[("P", Value::DateTime(date(serial)))],
                        Default::default(),
                    )
                    .unwrap();
                assert_eq!(result.rows(), &[vec![Value::Int32(expected)]]);
            }
            assert!(index_reads.get() > 0);
            assert!(data_reads.get() > 0);
            assert_eq!(
                run(&source, "SELECT Id FROM Items WHERE Score IS NULL")
                    .unwrap()
                    .rows(),
                &[vec![Value::Int32(1)]],
            );
            assert_eq!(
                run(&source, "SELECT Count(*) FROM Items").err(),
                Some(QueryError::StructureLimit)
            );
        }
    }
}

#[test]
fn temporal_residuals_keep_epoch_aliases_and_automatic_source_order() {
    let values = [
        None,
        Some(-0.25),
        Some(0.25),
        Some(0.0),
        Some(-0.0),
        Some(-1.25),
        Some(-1.75),
        Some(1.25),
    ];
    let sql = "SELECT Id FROM Items WHERE Score=#12/30/1899 06:00#";
    for profile in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = database(profile, descending, &values);
            let sequential = native(bytes.clone(), 100);
            let automatic = native(bytes.clone(), 100).with_auto_indexes();
            let explicit = native(bytes, 100).with_index("Items", "Compound").unwrap();
            assert_eq!(
                run(&sequential, sql).unwrap().rows(),
                &[vec![Value::Int32(2)], vec![Value::Int32(3)]]
            );
            assert_eq!(
                run(&automatic, sql).unwrap().rows(),
                run(&sequential, sql).unwrap().rows()
            );
            let expected = if descending { [3, 2] } else { [2, 3] };
            let indexed = run(&explicit, sql).unwrap();
            assert_eq!(indexed.rows(), expected.map(|id| vec![Value::Int32(id)]));

            let range =
                "SELECT Id FROM Items WHERE Score>=#12/29/1899 06:00# AND Score<#12/29/1899 18:00#";
            assert_eq!(
                run(&automatic, range).unwrap().rows(),
                &[vec![Value::Int32(6)]]
            );
        }
    }
}

#[test]
fn conservative_epoch_candidates_keep_their_explicit_retention_ceiling() {
    let source = native(
        database(6, false, &[Some(0.0), Some(0.25), Some(0.5), Some(1.25)]),
        1,
    )
    .with_auto_indexes();
    assert_eq!(
        run(
            &source,
            "SELECT Id FROM Items WHERE Score=#12/30/1899 12:00#"
        )
        .err(),
        Some(QueryError::StructureLimit),
    );
    assert_eq!(
        run(
            &source,
            "SELECT Id FROM Items WHERE Score=#12/31/1899 06:00#"
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(4)]],
    );
}

#[test]
fn corrupt_chosen_date_indexes_fail_but_unsafe_predicates_do_not_trigger_selection() {
    let mut bytes = database(6, false, &[None, Some(1.25)]);
    bytes[INDEX_PAGE * PAGE_BYTES] = 0;
    let sequential = native(bytes.clone(), 100);
    let automatic = native(bytes, 100).with_auto_indexes();
    let sql = "SELECT Id FROM Items WHERE Score=#12/31/1899 06:00#";
    assert_eq!(
        run(&sequential, sql).unwrap().rows(),
        &[vec![Value::Int32(2)]]
    );
    assert!(run(&automatic, sql).is_err());
    assert!(matches!(
        run(
            &automatic,
            "SELECT Id FROM Items WHERE Score=#12/31/1899 06:00# AND 1/0=0"
        ),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}
