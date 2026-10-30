#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

#[path = "recomputed_calculations/boundaries.rs"]
mod boundaries;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::{CalculationError, CalculationView, NativeSource, RecomputedNativeSource};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

struct Watching {
    bytes: Cursor<Vec<u8>>,
    forbidden: Rc<Cell<bool>>,
    reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Watching {}

impl Read for Watching {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if self.bytes.position() as usize / PAGE_BYTES == DATA_PAGE {
            assert!(
                !self.forbidden.get(),
                "metadata preparation read user records"
            );
            self.reads.set(self.reads.get() + 1);
        }
        self.bytes.read(output)
    }
}

impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

fn rows(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), source)?;
    Ok(plan
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn views_bind_without_records_and_compose_without_changing_original_cached_routes() {
    for profile in [5, 6] {
        let mut original = calculated_fixture::database(&[Some(999), None, Some(41)]);
        original[20] = profile;
        let forbidden = Rc::new(Cell::new(true));
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(original),
                forbidden: forbidden.clone(),
                reads: reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_stored_calculations()
        .unwrap();
        let views = RecomputedNativeSource::new(
            &source,
            &[CalculationView::new("Fresh", "items", &["id", "score"])],
            ParseLimits::default(),
        )
        .unwrap();
        let query = parse_query(
            "SELECT ID,Score FROM Fresh WHERE Score>2 ORDER BY ID",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(query.query(), &views).unwrap();
        assert_eq!(reads.get(), 0);
        assert_eq!(plan.output_names().unwrap(), ["ID", "Score"]);

        forbidden.set(false);
        assert_eq!(
            plan.execute(&views, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            &[
                vec![Value::Int32(2), Value::Int64(4)],
                vec![Value::Int32(3), Value::Int64(6)],
            ]
        );
        assert_eq!(
            rows(&views, "SELECT Score FROM Items ORDER BY Id").unwrap(),
            [
                vec![Value::Int32(999)],
                vec![Value::Null],
                vec![Value::Int32(41)]
            ]
        );
        assert_eq!(
            rows(
                &views,
                "SELECT F.Score FROM Fresh F INNER JOIN Items I ON F.ID=I.ID ORDER BY F.ID",
            )
            .unwrap(),
            [
                vec![Value::Int64(2)],
                vec![Value::Int64(4)],
                vec![Value::Int64(6)]
            ]
        );
    }
}

#[test]
fn unselected_formulas_stay_inert_but_selected_view_projections_remain_eager() {
    let source = NativeSource::new(
        Cursor::new(calculated_fixture::database_with_expression(
            &[None, None],
            "IIf(True,[Id],1/0)",
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let views = RecomputedNativeSource::new(
        &source,
        &[
            CalculationView::new("IDs", "Items", &["Id"]),
            CalculationView::new("Eager", "Items", &["Id", "Score"]),
        ],
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        rows(&views, "SELECT Id FROM IDs ORDER BY Id").unwrap(),
        [vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    for sql in [
        "SELECT Id FROM Eager WHERE False",
        "SELECT TOP 1 Id FROM Eager",
        "SELECT Count(*) FROM Eager",
    ] {
        assert!(rows(&views, sql).is_err(), "{sql}");
    }

    let unsupported = NativeSource::new(
        Cursor::new(calculated_fixture::database_with_expression(&[], "Now()")),
        CatalogScope::new(2),
        budget(),
        &|| false,
    )
    .unwrap();
    let ordinary = RecomputedNativeSource::new(
        &unsupported,
        &[CalculationView::new("IDs", "Items", &["Id"])],
        ParseLimits::default(),
    )
    .unwrap();
    assert!(rows(&ordinary, "SELECT * FROM IDs").unwrap().is_empty());
    assert!(matches!(
        RecomputedNativeSource::new(
            &unsupported,
            &[CalculationView::new("Fresh", "Items", &["Score"])],
            ParseLimits::default(),
        ),
        Err(CalculationError::Unsupported)
    ));
}
