use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::values::{Decimal, Double, LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

#[path = "date_average/bounds.rs"]
mod bounds;

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

struct Dates {
    rows: MemorySource,
    view: Query,
    scans: Cell<usize>,
}

impl Dates {
    fn new(values: Vec<Value>) -> Self {
        Self {
            rows: MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![Column::new("Moment".into(), ScalarType::DateTime, true).unwrap()],
                    values.into_iter().map(|value| vec![value]).collect(),
                )
                .unwrap(),
            ])
            .unwrap(),
            view: parse_query("SELECT Moment FROM T", Default::default())
                .unwrap()
                .into_query(),
            scans: Cell::new(0),
        }
    }
}

impl Source for Dates {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.rows.columns(name)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.view))
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.rows.scan(name, visitor)
    }
}

fn run(source: &Dates, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn date_means_keep_numeric_serials_and_prepare_without_scanning() {
    for (values, expected) in [
        (vec![date(2.25), Value::Null, date(-1.25)], double(0.5)),
        (vec![date(-0.25), date(0.25)], double(0.0)),
        (vec![date(-0.0)], double(0.0)),
        (vec![Value::Null], Value::Null),
        (vec![], Value::Null),
    ] {
        for table in ["T", "V"] {
            let source = Dates::new(values.clone());
            let parsed = parse_query(
                &format!("SELECT Avg(Moment) FROM {table}"),
                Default::default(),
            )
            .unwrap();
            let prepared = prepare(parsed.query(), &source).unwrap();
            assert_eq!(source.scans.get(), 0);
            assert_eq!(
                prepared.output_types().unwrap(),
                [OutputType::Scalar {
                    data_type: TypeDescriptor::new(ScalarType::Double),
                    nullable: true,
                }]
            );
            assert_eq!(
                prepared
                    .execute(&source, &[], Default::default())
                    .unwrap()
                    .rows(),
                [vec![expected.clone()]]
            );
            assert_eq!(
                run(&source, &format!("SELECT DAvg('Moment','{table}')")).unwrap(),
                [vec![expected.clone()]]
            );
        }
    }
}

#[test]
fn date_means_compose_with_numeric_consumers_groups_and_crosstabs() {
    let source = Dates::new(vec![date(2.25), Value::Null, date(-1.25)]);
    for sql in [
        "SELECT Avg(Moment) FROM T HAVING Avg(Moment)>0",
        "SELECT D.M FROM (SELECT Avg(Moment) AS M FROM V) AS D",
        "SELECT (SELECT Avg(Moment) FROM T)",
        "SELECT Avg(Moment) FROM T GROUP BY True",
    ] {
        assert_eq!(run(&source, sql).unwrap(), [vec![double(0.5)]]);
    }
    assert_eq!(
        run(
            &source,
            "SELECT Avg(Moment) AS M FROM T UNION ALL SELECT CDbl(1)"
        )
        .unwrap(),
        [vec![double(0.5)], vec![double(1.0)]]
    );
    assert_eq!(
        run(
            &source,
            "TRANSFORM Avg(Moment) SELECT True AS G FROM T GROUP BY True PIVOT True IN (True,False)"
        )
        .unwrap(),
        [vec![Value::Boolean(true), double(0.5), Value::Null]]
    );

    // Explicit Decimal still selects the existing Decimal mean profile.
    for (condition, coefficient) in [("Moment>#1899-12-30#", 150), ("Moment<#1899-12-30#", 325)] {
        let expected = Decimal::new(coefficient, 2).unwrap();
        let sql = format!(
            "SELECT Avg(IIf({condition},CDec(4.25),Moment)) FROM T WHERE Moment IS NOT NULL"
        );
        let actual = run(&source, &sql).unwrap();
        let Value::Decimal(actual) = actual[0][0] else {
            panic!("expected Decimal mean");
        };
        assert_eq!(actual.rescale_exact(2).unwrap(), expected);
    }
}

#[test]
fn date_average_admission_does_not_extend_sum_or_convert_unsupported_cells() {
    let source = Dates::new(vec![date(1.0), date(2.0)]);
    for sql in [
        "SELECT Sum(Moment) FROM T",
        "SELECT Avg('1.25') FROM T",
        "SELECT Avg(CVErr(5)) FROM T",
    ] {
        assert_eq!(run(&source, sql), Err(EvalError::TypeMismatch.into()));
    }
    assert_eq!(
        run(&source, "SELECT Avg(Moment+1/0) FROM T"),
        Err(EvalError::DivisionByZero.into())
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4441_5445_4d45_414e),
        failure_persistence: None,
        ..Default::default()
    })]

    #[test]
    fn quarter_day_means_match_an_independent_integer_total(
        values in proptest::collection::vec(proptest::option::of(-4000_i16..4000), 0..32)
    ) {
        let total: i64 = values.iter().flatten().map(|value| i64::from(*value)).sum();
        let count = values.iter().flatten().count();
        let expected = if count == 0 {
            Value::Null
        } else {
            double(total as f64 / (4 * count) as f64)
        };
        let rows = values
            .iter()
            .map(|value| value.map_or(Value::Null, |value| date(f64::from(value) / 4.0)))
            .collect();
        let source = Dates::new(rows);
        let actual = run(&source, "SELECT Avg(Moment),DAvg('Moment','T') FROM T").unwrap();
        prop_assert_eq!(actual, [vec![expected.clone(), expected]]);
    }
}
