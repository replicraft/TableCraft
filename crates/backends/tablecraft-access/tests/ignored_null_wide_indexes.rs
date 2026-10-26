#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ignored_null_indexes/fixture.rs"]
mod ignored;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
#[path = "../../../engines/tablecraft-ace/tests/ignored_null_wide_indexes/fixture.rs"]
mod wide;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, QueryResult, Source, prepare};
use tablecraft_sql::parse_query;

const VALUES: &[Option<i64>] = &[
    Some(20_000),
    None,
    Some(10_000),
    Some(30_000),
    None,
    Some(20_000),
];

struct Reads {
    cursor: Cursor<Vec<u8>>,
    index_reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Reads {}

impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor.position() == (wide::ROOT * wide::PAGE_BYTES) as u64 {
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

fn codes(currency: bool) -> &'static [u8] {
    if currency { &[1, 2, 5, 6] } else { &[5, 6] }
}

fn run(
    source: &dyn Source,
    sql: &str,
    parameters: &[(&str, Value)],
) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, parameters, Default::default())
}

fn ids(values: &[i32]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|value| vec![Value::Int32(*value)])
        .collect()
}

fn point(currency: bool, value: i64) -> Value {
    if currency {
        Value::Currency(Currency::from_scaled(value))
    } else {
        Value::Int64(value)
    }
}

fn sql(currency: bool, predicate: &str) -> String {
    let kind = if currency { "CURRENCY" } else { "BIGINT" };
    format!("PARAMETERS P {kind}; SELECT Id FROM Items WHERE {predicate}")
}

#[test]
fn automatic_and_required_wide_reads_preserve_separate_encounter_and_key_order() {
    for currency in [false, true] {
        for &code in codes(currency) {
            for descending in [false, true] {
                let bytes = wide::database(code, currency, descending, true, VALUES);
                let source = NativeSource::new(
                    Cursor::new(bytes.clone()),
                    CatalogScope::new(1),
                    budget(),
                    &|| false,
                )
                .unwrap()
                .with_auto_indexes();
                let query = sql(currency, "Score>=P");
                assert_eq!(
                    run(&source, &query, &[("P", point(currency, 10_000))])
                        .unwrap()
                        .rows(),
                    ids(&[1, 3, 4, 6])
                );
                let required =
                    NativeSource::new(Cursor::new(bytes), CatalogScope::new(2), budget(), &|| {
                        false
                    })
                    .unwrap()
                    .with_index("Items", "BigKey")
                    .unwrap();
                let expected = if descending {
                    &[4, 1, 6, 3]
                } else {
                    &[3, 1, 6, 4]
                };
                assert_eq!(
                    run(&required, &query, &[("P", point(currency, 10_000))])
                        .unwrap()
                        .rows(),
                    ids(expected)
                );
                for query in [
                    "SELECT Id FROM Items",
                    "SELECT Id FROM Items WHERE Score IS NULL",
                ] {
                    assert_eq!(
                        run(&required, query, &[]).err(),
                        Some(QueryError::Unsupported)
                    );
                }
            }
        }
    }
}

#[test]
fn exact_typed_wide_parameters_select_extremes_without_partial_cache_promotion() {
    let values = [
        Some(i64::MIN),
        None,
        Some(9_007_199_254_740_992),
        Some(9_007_199_254_740_993),
        Some(i64::MAX),
    ];
    for currency in [false, true] {
        for &code in codes(currency) {
            let reads = Rc::new(Cell::new(0));
            let source = NativeSource::new_with_snapshot_limits(
                Reads {
                    cursor: Cursor::new(wide::database(code, currency, false, true, &values)),
                    index_reads: reads.clone(),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, 1).unwrap(),
            )
            .unwrap()
            .with_auto_indexes();
            let parsed = parse_query(&sql(currency, "Score=P"), Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!(reads.get(), 0);
            for (value, id) in [
                (i64::MIN, 1),
                (9_007_199_254_740_992, 3),
                (9_007_199_254_740_993, 4),
                (i64::MAX, 5),
            ] {
                assert_eq!(
                    plan.execute(
                        &source,
                        &[("P", point(currency, value))],
                        Default::default()
                    )
                    .unwrap()
                    .rows(),
                    ids(&[id])
                );
            }
            assert!(reads.get() > 0);
            let before = reads.get();
            for query in [
                "SELECT Count(*) FROM Items",
                "SELECT Id FROM Items WHERE Score IS NULL",
            ] {
                assert_eq!(
                    run(&source, query, &[]).err(),
                    Some(QueryError::StructureLimit)
                );
            }
            assert_eq!(reads.get(), before);
        }
    }
}

#[test]
fn chosen_wide_corruption_propagates_but_omitted_null_scans_are_independent() {
    for currency in [false, true] {
        for &code in codes(currency) {
            let mut bytes = wide::database(code, currency, false, false, VALUES);
            bytes[wide::ROOT * wide::PAGE_BYTES] = 0;
            let reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Reads {
                    cursor: Cursor::new(bytes.clone()),
                    index_reads: reads.clone(),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap()
            .with_auto_indexes();
            assert_eq!(
                run(&source, "SELECT Id FROM Items WHERE Score IS NULL", &[])
                    .unwrap()
                    .rows(),
                ids(&[2, 5])
            );
            assert_eq!(reads.get(), 0);
            let selected =
                NativeSource::new(Cursor::new(bytes), CatalogScope::new(2), budget(), &|| {
                    false
                })
                .unwrap()
                .with_auto_indexes();
            assert_eq!(
                run(
                    &selected,
                    &sql(currency, "Score=P"),
                    &[("P", point(currency, 20_000))]
                )
                .err(),
                Some(QueryError::Source)
            );
            assert_eq!(
                run(&selected, "SELECT Count(*) FROM Items", &[])
                    .unwrap()
                    .rows(),
                [vec![Value::Int64(6)]]
            );
        }
    }
}

#[test]
fn wide_hint_extraction_keeps_eager_errors_and_existing_closed_currency_points() {
    for currency in [false, true] {
        let source = NativeSource::new(
            Cursor::new(wide::database(5, currency, false, false, VALUES)),
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_auto_indexes();
        assert_eq!(
            run(
                &source,
                &sql(currency, "Score=P AND 1/0=0"),
                &[("P", point(currency, i64::MAX))]
            )
            .err(),
            Some(EvalError::DivisionByZero.into())
        );
        if currency {
            assert_eq!(
                run(&source, "SELECT Id FROM Items WHERE Score=CCur(2)", &[])
                    .unwrap()
                    .rows(),
                ids(&[1, 6])
            );
        }
    }
}
