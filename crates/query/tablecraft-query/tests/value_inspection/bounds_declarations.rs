//! Existing Long return signatures determine the explicit arithmetic category.
use super::source;
use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, Source, prepare,
    prepare_with_options,
};
use tablecraft_schema::Column;
use tablecraft_sql::{parse_query, query::Query};

fn execute(sql: &str, source: &dyn Source, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = prepare(parsed.query(), source)?.execute(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        },
    )?;
    Ok(result.rows().to_vec())
}

#[test]
fn bound_long_returns_overflow_as_scalars_and_explicit_variants_widen() {
    let source = source();
    for (bound, operator, limit) in [
        ("UBound(Array(1,2))", "+", "CLng(2147483647)"),
        ("LBound(Array(1,2))", "-", "CLng(-2147483648)"),
    ] {
        let sql = format!("SELECT {bound}{operator}{limit}");
        assert_eq!(
            execute(&sql, &source, true),
            Err(EvalError::Overflow.into())
        );
        assert_eq!(
            execute(&sql, &source, false).unwrap(),
            [vec![Value::Double(Double::from_float(2147483648.0))]]
        );

        let sql = format!("SELECT CVar({bound}){operator}{limit}");
        for declared in [false, true] {
            assert_eq!(
                execute(&sql, &source, declared).unwrap(),
                [vec![Value::Double(Double::from_float(2147483648.0))]]
            );
        }
    }
}

struct Named {
    data: tablecraft_query::source::MemorySource,
    query: Query,
    scans: Cell<usize>,
}

impl Source for Named {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Bounds").then_some(&self.query))
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(name, visitor)
    }
}

#[test]
fn named_derived_and_correlated_bound_categories_bind_without_scanning() {
    let source = Named {
        data: source(),
        query: parse_query("SELECT UBound(B) AS N FROM T", Default::default())
            .unwrap()
            .into_query(),
        scans: Cell::new(0),
    };
    for sql in [
        "SELECT N+CLng(2147483647) FROM Bounds",
        "SELECT Q.N+CLng(2147483647) FROM (SELECT N FROM Bounds) Q",
        "SELECT (SELECT Q.N)+CLng(2147483647) FROM (SELECT N FROM Bounds) Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let before = source.scans.get();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), before);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
        assert_eq!(source.scans.get(), before + 1);
    }
}

#[test]
fn fixed_pivot_heading_arithmetic_runs_before_scans_without_widening_constant_admission() {
    let mut source = Named {
        data: source(),
        query: parse_query("SELECT 1 AS N", Default::default())
            .unwrap()
            .into_query(),
        scans: Cell::new(0),
    };
    for (heading, overflow) in [
        ("UBound(Array(1,2))+CLng(2147483647)", true),
        ("CVar(UBound(Array(1,2)))+CLng(2147483647)", false),
    ] {
        let sql = format!("TRANSFORM Count(*) SELECT N FROM T GROUP BY N PIVOT N IN ({heading})");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        source.scans.set(0);
        let plan = prepare_with_options(
            parsed.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(source.scans.get(), 0);

        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        );
        if overflow {
            assert_eq!(result.err(), Some(EvalError::Overflow.into()));
            assert_eq!(source.scans.get(), 0);
        } else {
            let result = result.unwrap();
            assert_eq!(result.columns(), ["N", "2147483648"]);
            assert_eq!(
                result.rows(),
                [
                    vec![Value::Int32(1), Value::Null],
                    vec![Value::Int32(2), Value::Null],
                    vec![Value::Int32(4), Value::Null],
                ]
            );
            assert_eq!(source.scans.get(), 1);
        }

        // Array bounds remain outside metadata-only constant heading folding.
        source.query = parsed.into_query();
        source.scans.set(0);
        let named = parse_query("SELECT * FROM Bounds", Default::default()).unwrap();
        assert_eq!(
            prepare(named.query(), &source).err(),
            Some(QueryError::Unsupported)
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn union_and_aggregate_outputs_keep_their_unqualified_categories() {
    let source = source();
    for sql in [
        "SELECT N+CLng(2147483647) FROM (SELECT UBound(Array(1,2)) AS N UNION ALL SELECT UBound(Array(1,2)) WHERE False) Q",
        "SELECT First(UBound(B))+CLng(2147483647) FROM T",
        "SELECT Min(UBound(B))+CLng(2147483647) FROM T",
    ] {
        assert_eq!(
            execute(sql, &source, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
}

#[test]
fn empty_results_and_eager_array_dimension_failures_keep_their_existing_outcomes() {
    let source = source();
    assert_eq!(
        execute(
            "SELECT (SELECT UBound(Array(1,2)) WHERE False)+CLng(2147483647)",
            &source,
            true
        )
        .unwrap(),
        [vec![Value::Null]]
    );
    for (sql, error) in [
        (
            "SELECT UBound(Array(1/0))+CLng(2147483647)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT UBound(NULL,1/0)+CLng(2147483647)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT UBound(NULL)+CLng(2147483647)",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT LBound(Array(1),2)+CLng(2147483647)",
            EvalError::ArrayBounds,
        ),
        ("SELECT UBound(Array(1,2))+(1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(execute(sql, &source, true), Err(error.into()), "{sql}");
    }
}

#[test]
fn declared_bound_arithmetic_keeps_cancellation_and_query_limits() {
    let source = source();
    let parsed = parse_query("SELECT UBound(B)+CLng(1) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            declared_integral: true,
            cancelled: Some(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let error = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    cancelled: Some(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    ..Default::default()
                },
            )
            .err();
        assert!(
            matches!(
                error,
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}: {error:?}"
        );
    }

    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        let error = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    limits,
                    ..Default::default()
                },
            )
            .err();
        assert!(matches!(
            error,
            Some(QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_)))
        ));
    }
}
