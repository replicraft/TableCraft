use super::*;
use std::ops::Bound::{Included, Unbounded};
use tablecraft_query::source::ScanConstraint;

struct RootReads {
    cursor: Cursor<Vec<u8>>,
    roots: Rc<Cell<usize>>,
    index_page: u32,
    index_reads: Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for RootReads {}
impl Read for RootReads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        // Independent fixture fact, not an offset imported from the decoder.
        if self.cursor.position() == 728 * 4096 {
            self.roots.set(self.roots.get() + 1);
        }
        if self.cursor.position() == u64::from(self.index_page) * 4096 {
            self.index_reads.set(self.index_reads.get() + 1);
        }
        self.cursor.read(bytes)
    }
}
impl Seek for RootReads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}
fn execute(sql: &str, source: &dyn Source) -> Result<tablecraft_query::QueryResult, QueryError> {
    let parsed = tablecraft_sql::parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, &[], ExecutionOptions::default())
}

#[test]
fn query_seeks_only_matching_roots_and_never_caches_a_subset_as_the_table() {
    let roots = Rc::new(Cell::new(0));
    let index_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        RootReads {
            cursor: Cursor::new(copy()),
            roots: roots.clone(),
            index_page: 731,
            index_reads: index_reads.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("ROULETTE_TB", "ORDR_NBR")
    .unwrap();
    assert_eq!(source.columns("ROULETTE_TB").unwrap().len(), 3);
    assert_eq!(
        (roots.get(), index_reads.get()),
        (0, 0),
        "schema discovery fetched data/index records"
    );
    let result = execute("SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR >= 10 AND ORDR_NBR <= 12 ORDER BY ORDR_NBR DESC", &source).unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![Value::Int32(12)],
            vec![Value::Int32(11)],
            vec![Value::Int32(10)]
        ]
    );
    assert_eq!(
        (roots.get(), index_reads.get()),
        (1, 1),
        "selected roots share one page and retain required index traversal"
    );
    assert_eq!(
        execute("SELECT Count(*) FROM ROULETTE_TB", &source)
            .unwrap()
            .rows(),
        &[vec![Value::Int64(38)]]
    );
    assert_eq!((roots.get(), index_reads.get()), (2, 2));
    let cached = (roots.get(), index_reads.get());
    assert_eq!(
        execute(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(4)]]
    );
    assert_eq!(
        (roots.get(), index_reads.get()),
        (cached.0 + 1, cached.1 + 1),
        "a cached count projection has no ordinary value cells"
    );
    execute("SELECT ORDR_NBR FROM ROULETTE_TB", &source).unwrap();
    let complete = (roots.get(), index_reads.get());
    execute(
        "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
        &source,
    )
    .unwrap();
    assert_eq!(
        (roots.get(), index_reads.get()),
        complete,
        "complete matching projections supply residual-filtered supersets"
    );
}

#[test]
fn unrelated_constraints_use_the_required_full_index_and_preserve_errors() {
    let roots = Rc::new(Cell::new(0));
    let index_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        RootReads {
            cursor: Cursor::new(copy()),
            roots: roots.clone(),
            index_page: 729,
            index_reads: index_reads.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("ROULETTE_TB", "COLOR_ID")
    .unwrap();
    assert_eq!(
        execute(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(4)]]
    );
    assert_eq!((roots.get(), index_reads.get()), (1, 1));
    assert_eq!(
        execute("SELECT Count(*) FROM ROULETTE_TB", &source)
            .unwrap()
            .rows(),
        &[vec![Value::Int64(38)]]
    );
    assert_eq!(
        (roots.get(), index_reads.get()),
        (1, 1),
        "unrelated predicates loaded and cached the complete required index"
    );
    let text_source =
        NativeSource::new(Cursor::new(copy()), CatalogScope::new(7), budget(), &|| {
            false
        })
        .unwrap()
        .with_index("ROULETTE_TB", "Number")
        .unwrap();
    assert_eq!(
        execute(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
            &text_source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(4)]]
    );
    {
        let source =
            NativeSource::new(Cursor::new(copy()), CatalogScope::new(7), budget(), &|| {
                false
            })
            .unwrap()
            .with_index("ROULETTE_TB", "missing")
            .unwrap();
        assert_eq!(
            execute(
                "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
                &source
            )
            .err(),
            Some(QueryError::UnknownName)
        );
    }
    let mut bytes = copy();
    bytes[731 * 4096] = 0;
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(7), budget(), &|| {
        false
    })
    .unwrap()
    .with_index("ROULETTE_TB", "ORDR_NBR")
    .unwrap();
    assert_eq!(
        execute(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR = 4",
            &source
        )
        .err(),
        Some(QueryError::Source)
    );
}

#[test]
fn constrained_scans_stop_on_visitor_failure_and_keep_cancellation_and_retry_budgets() {
    let fail = Rc::new(Cell::new(false));
    let cancel = Cell::new(false);
    let callback = || cancel.get();
    let source = NativeSource::new(
        SwitchReader {
            cursor: Cursor::new(copy()),
            fail: fail.clone(),
            reads: Rc::new(Cell::new(0)),
        },
        CatalogScope::new(7),
        budget(),
        &callback,
    )
    .unwrap()
    .with_index("ROULETTE_TB", "ORDR_NBR")
    .unwrap();
    let constraint = ScanConstraint::Int32 {
        column: 0,
        lower: Included(10),
        upper: Unbounded,
    };
    fail.set(true);
    assert_eq!(
        source.scan_constrained("ROULETTE_TB", constraint, &mut |_| panic!("partial row")),
        Err(QueryError::Source)
    );
    fail.set(false);
    let mut visited = 0;
    assert_eq!(
        source.scan_constrained("ROULETTE_TB", constraint, &mut |_| {
            visited += 1;
            Err(QueryError::Cardinality)
        }),
        Err(QueryError::Cardinality)
    );
    assert_eq!(visited, 1);
    cancel.set(true);
    assert_eq!(
        source.scan_constrained("ROULETTE_TB", constraint, &mut |_| panic!("cancelled row")),
        Err(QueryError::Cancelled)
    );
}
