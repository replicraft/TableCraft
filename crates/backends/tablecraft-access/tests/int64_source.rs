#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "../../../engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    ops::Bound::Included,
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryResult, Source, prepare, source::ScanConstraint,
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};

struct Reads {
    source: Cursor<Vec<u8>>,
    data: Rc<Cell<usize>>,
    index: Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for Reads {}
impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.source.position() == 12 * 4096 {
            self.data.set(self.data.get() + 1);
        }
        if self.source.position() == 14 * 4096 {
            self.index.set(self.index.get() + 1);
        }
        self.source.read(bytes)
    }
}
impl Seek for Reads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(position)
    }
}
fn execute(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn bytes(code: u8, descending: bool) -> Vec<u8> {
    int64_index_fixture::database(
        code,
        descending,
        &[
            Some(i64::MIN),
            Some(9_007_199_254_740_992),
            Some(9_007_199_254_740_993),
            Some(i64::MAX),
            None,
            None,
            Some(-1),
            Some(1),
        ],
    )
}

#[test]
fn int64_query_seeks_fetch_only_matching_roots_and_keep_full_cache_separate() {
    for code in [5, 6] {
        for descending in [false, true] {
            let count = Rc::new(Cell::new(0));
            let index_reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Reads {
                    source: Cursor::new(bytes(code, descending)),
                    data: count.clone(),
                    index: index_reads.clone(),
                },
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap()
            .with_index("Items", "BigKey")
            .unwrap();
            source.columns("Items").unwrap();
            assert_eq!((count.get(), index_reads.get()), (0, 0));
            let parsed = parse_query(
                "SELECT Id,Score FROM Items WHERE Score=9007199254740993",
                ParseLimits::default(),
            )
            .unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!((count.get(), index_reads.get()), (0, 0));
            for (kind, expected) in plan
                .output_types()
                .unwrap()
                .iter()
                .zip([ScalarType::Int32, ScalarType::Int64])
            {
                let OutputType::Scalar { data_type, .. } = kind else {
                    panic!("ACE 16 field metadata must retain its declared carrier");
                };
                assert_eq!(data_type.scalar_type(), expected);
            }
            let result = plan
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            assert_eq!(result.output_types(), plan.output_types().unwrap());
            assert_eq!(
                result.rows(),
                &[vec![Value::Int32(3), Value::Int64(9_007_199_254_740_993)]]
            );
            assert_eq!((count.get(), index_reads.get()), (1, 1));
            assert_eq!(
                execute(
                    "SELECT Id FROM Items WHERE Score IS NULL ORDER BY Id",
                    &source
                )
                .unwrap()
                .rows(),
                &[vec![Value::Int32(5)], vec![Value::Int32(6)]]
            );
            assert_eq!((count.get(), index_reads.get()), (2, 2));
            assert_eq!(execute("SELECT Id FROM Items WHERE Score>9007199254740992 AND Score<9223372036854775807",&source).unwrap().rows(),&[vec![Value::Int32(3)]]);
            assert_eq!((count.get(), index_reads.get()), (3, 3));
            assert_eq!(
                execute("SELECT Count(*) FROM Items", &source)
                    .unwrap()
                    .rows(),
                &[vec![Value::Int64(8)]]
            );
            assert_eq!((count.get(), index_reads.get()), (4, 4));
            execute("SELECT Id FROM Items WHERE Score=-1", &source).unwrap();
            assert_eq!(
                (count.get(), index_reads.get()),
                (5, 5),
                "count-only cache does not contain Id or Score values"
            );
            execute("SELECT Id, Score FROM Items", &source).unwrap();
            let complete = (count.get(), index_reads.get());
            execute("SELECT Id FROM Items WHERE Score=-1", &source).unwrap();
            assert_eq!(
                (count.get(), index_reads.get()),
                complete,
                "full cache supplies residual-filtered supersets"
            );
        }
    }
}

#[test]
fn int64_sources_preserve_required_index_errors_and_decline_wrong_width_hints() {
    let source = NativeSource::new(
        Cursor::new(bytes(6, false)),
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "missing")
    .unwrap();
    assert_eq!(
        execute("SELECT Id FROM Items WHERE Score=1", &source).err(),
        Some(QueryError::UnknownName)
    );
    let mut corrupt = bytes(6, false);
    corrupt[14 * 4096] = 0;
    let source = NativeSource::new(
        Cursor::new(corrupt),
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "BigKey")
    .unwrap();
    assert_eq!(
        execute("SELECT Id FROM Items WHERE Score=1", &source).err(),
        Some(QueryError::Source)
    );
    let count = Rc::new(Cell::new(0));
    let index_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Reads {
            source: Cursor::new(bytes(6, false)),
            data: count.clone(),
            index: index_reads.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "BigKey")
    .unwrap();
    let mut rows = 0;
    source
        .scan_constrained(
            "Items",
            ScanConstraint::Int32 {
                column: 1,
                lower: Included(1),
                upper: Included(1),
            },
            &mut |_| {
                rows += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        (rows, count.get(), index_reads.get()),
        (8, 1, 1),
        "wrong-width hint retains required complete index path"
    );
}

#[test]
fn int64_partial_scans_keep_visitor_and_cancellation_failures_observable() {
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let source = NativeSource::new(
        Cursor::new(bytes(6, false)),
        CatalogScope::new(7),
        budget(),
        &callback,
    )
    .unwrap()
    .with_index("Items", "BigKey")
    .unwrap();
    let selection = ScanConstraint::Int64 {
        column: 1,
        lower: Included(9_007_199_254_740_993),
        upper: Included(i64::MAX),
    };
    let mut visited = 0;
    assert_eq!(
        source.scan_constrained("Items", selection, &mut |_| {
            visited += 1;
            Err(QueryError::Cardinality)
        }),
        Err(QueryError::Cardinality)
    );
    assert_eq!(visited, 1);
    cancelled.set(true);
    assert_eq!(
        source.scan_constrained("Items", selection, &mut |_| panic!("cancelled visitor")),
        Err(QueryError::Cancelled)
    );
}

#[test]
fn ace16_currency_averages_use_wide_totals_and_keep_binding_metadata_only() {
    for code in [5, 6] {
        for (values, expected) in [
            (vec![Some(12500_i64), None, Some(25000)], 1.875),
            (vec![Some(i64::MAX), Some(i64::MAX)], 922337203685477.6),
        ] {
            let encoded = values
                .iter()
                .map(|value| value.map(|n| n.to_le_bytes().to_vec()))
                .collect::<Vec<_>>();
            let count = Rc::new(Cell::new(0));
            let index_reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Reads {
                    source: Cursor::new(scalar_fixture::fixed_scalar(code, 5, 8, &encoded)),
                    data: count.clone(),
                    index: index_reads.clone(),
                },
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap();
            let parsed = parse_query(
                "SELECT Avg(Score), DAvg('Score','Items') FROM Items",
                ParseLimits::default(),
            )
            .unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!((count.get(), index_reads.get()), (0, 0));
            let result = plan
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            let value = Value::Double(Double::from_float(expected));
            assert_eq!(result.rows(), [vec![value.clone(), value]]);
        }
    }
}

#[test]
fn ace16_floating_means_keep_finite_results_and_tiny_cancellation_residuals() {
    for code in [5, 6] {
        for (values, expected) in [
            (vec![f64::MAX, f64::MAX], f64::MAX),
            (vec![1e308, 1e-300, -1e308], 1e-300 / 3.0),
        ] {
            let encoded = values
                .iter()
                .map(|value| Some(value.to_le_bytes().to_vec()))
                .collect::<Vec<_>>();
            let count = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Reads {
                    source: Cursor::new(scalar_fixture::fixed_scalar(code, 7, 8, &encoded)),
                    data: count.clone(),
                    index: Rc::new(Cell::new(0)),
                },
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap();
            let parsed = parse_query(
                "SELECT Avg(Score), DAvg('Score','Items') FROM Items",
                ParseLimits::default(),
            )
            .unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!(count.get(), 0);
            let result = plan
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            for value in &result.rows()[0] {
                let Value::Double(value) = value else {
                    panic!("missing native Double mean");
                };
                assert_eq!(value.to_float().to_bits(), expected.to_bits());
            }
        }
    }
}
