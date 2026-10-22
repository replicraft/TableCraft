use super::*;
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::functions::{conversions::text::NumberFormat, math::random::RndState};
use tablecraft_query::{ExpressionQueryResult, execute_with_schema_discovery};

const DYNAMIC: &str = "TRANSFORM Sum(Amount) SELECT Region,Sum(Amount) AS Total FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter";

fn run(
    source: &Views,
    sql: &str,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'_>,
) -> Result<ExpressionQueryResult, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    execute_with_schema_discovery(query.query(), source, parameters, options)
}

fn row(values: Vec<Value>) -> Vec<ExpressionValue> {
    values.into_iter().map(ExpressionValue::Value).collect()
}

#[test]
fn explicit_discovery_materializes_dynamic_columns_once_per_run() {
    let source = Views::new(&[("Piv", DYNAMIC)]);
    let sql = "SELECT A.Region,A.[2],B.[3] FROM Piv A INNER JOIN Piv B ON A.Region=B.Region ORDER BY A.Region";
    let query = parse_query(sql, Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(source.scans.get(), 0);
    for attempt in 1..=2 {
        let result = run(&source, sql, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            [
                row(vec![Value::Int32(1), Value::Int64(17), Value::Null]),
                row(vec![Value::Int32(2), Value::Null, Value::Int64(5)]),
                row(vec![Value::Int32(3), Value::Null, Value::Null]),
            ]
        );
        assert_eq!(source.scans.get(), attempt);
    }
}

#[test]
fn parameter_and_contextual_headings_use_the_current_run_and_isolated_scopes() {
    let source = Views::new(&[(
        "Piv",
        "PARAMETERS P TEXT; TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (CLng(P))",
    )]);
    for (value, format, heading, first) in [
        ("2.5", NumberFormat::EnUs, "2", Value::Int64(17)),
        ("3,5", NumberFormat::DeDe, "4", Value::Null),
    ] {
        let bindings = [("P", Value::Text(value.into())), ("Floor", Value::Int32(1))];
        let result = run(
            &source,
            "PARAMETERS Floor LONG; SELECT * FROM Piv WHERE Region>=Floor ORDER BY Region",
            &bindings,
            ExecutionOptions {
                number_format: Some(format),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.columns(), ["Region", heading]);
        assert_eq!(result.rows()[0], row(vec![Value::Int32(1), first]));
    }
    assert!(matches!(
        run(
            &source,
            "SELECT * FROM Piv",
            &[("P", Value::Text("2".into()))],
            Default::default()
        ),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    ));
}

#[test]
fn nested_discovery_keeps_cycles_and_complete_inner_errors_visible() {
    let source = Views::new(&[
        ("B", DYNAMIC),
        (
            "A",
            "TRANSFORM Sum([2]) SELECT Region FROM B GROUP BY Region ORDER BY Region PIVOT Region",
        ),
    ]);
    let result = run(
        &source,
        "SELECT Region,[1] FROM A ORDER BY Region",
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows()[0],
        row(vec![Value::Int32(1), Value::Int64(17)])
    );
    assert_eq!(source.scans.get(), 1);
    let source = Views::new(&[
        (
            "A",
            "TRANSFORM Count(*) SELECT Region FROM B GROUP BY Region PIVOT Region",
        ),
        ("B", "SELECT Region FROM A"),
    ]);
    assert_eq!(
        run(&source, "SELECT * FROM A", &[], Default::default()).err(),
        Some(QueryError::DependencyCycle)
    );
    assert_eq!(source.scans.get(), 0);
    let sql = DYNAMIC.replace("Sum(Amount)", "Sum(1/(Quarter-3))");
    let source = Views::new(&[("Piv", &sql)]);
    assert!(matches!(
        run(
            &source,
            "SELECT TOP 1 Region FROM Piv WHERE Region=1",
            &[],
            Default::default()
        ),
        Err(QueryError::Evaluation(_))
    ));
}

#[test]
fn expression_errors_and_volatile_cells_are_retained_without_reexecution() {
    let sql = "TRANSFORM First(CVErr(2001)) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter";
    let source = Views::new(&[("Piv", sql)]);
    let result = run(
        &source,
        "SELECT IsError(P.[2]) FROM Piv P WHERE Region=1",
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.rows(), [row(vec![Value::Boolean(true)])]);
    let sql = "TRANSFORM First(Rnd()) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter";
    let source = Views::new(&[("Piv", sql)]);
    let random = RndState::default();
    let result = run(
        &source,
        "SELECT A.[2],B.[2] FROM Piv A INNER JOIN Piv B ON A.Region=B.Region WHERE A.Region=1",
        &[],
        ExecutionOptions {
            random: Some(&random),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.rows()[0][0], result.rows()[0][1]);
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn discovery_and_reuse_share_row_limits_and_cancellation() {
    let source = Views::new(&[("Piv", DYNAMIC)]);
    let top = parse_query(DYNAMIC, Default::default()).unwrap();
    let plan = prepare(top.query(), &source).unwrap();
    let direct_minimum = (1..=128)
        .find(|rows| {
            plan.execute_expression(
                &source,
                &[],
                ExecutionOptions {
                    limits: QueryLimits {
                        rows: *rows,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .is_ok()
        })
        .unwrap();
    let result = run(
        &source,
        "SELECT * FROM Piv",
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                rows: direct_minimum,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(QueryError::Limit(error)) if error.resource == tablecraft_core::limits::Resource::Rows)
    );
    assert!(run(&source, "SELECT * FROM Piv", &[], Default::default()).is_ok());
    let calls = Cell::new(0);
    let invoke = |stop| {
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        run(
            &source,
            "SELECT * FROM Piv",
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    assert!(invoke(usize::MAX).is_ok());
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(
            matches!(
                invoke(stop),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn empty_inputs_keep_declared_contextual_columns_and_dynamic_row_headings() {
    let mut source = Views::new(&[
        ("Piv", DYNAMIC),
        (
            "Context",
            "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (CLng('2'))",
        ),
    ]);
    let columns = source.data.columns("Sales").unwrap().to_vec();
    source.data = MemorySource::new(vec![
        MemoryTable::new("Sales".into(), columns, Vec::new()).unwrap(),
    ])
    .unwrap();
    let empty = run(&source, "SELECT * FROM Piv", &[], Default::default()).unwrap();
    assert_eq!(empty.columns(), ["Region", "Total"]);
    assert!(empty.rows().is_empty());
    let empty = run(
        &source,
        "SELECT * FROM Context",
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::EnUs),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(empty.columns(), ["Region", "2"]);
    assert!(empty.rows().is_empty());
}

struct Changing {
    inner: Views,
    alternate: Query,
    fail: Cell<bool>,
}
impl Source for Changing {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        if name.eq_ignore_ascii_case("Piv") && self.inner.scans.get() > 0 && !self.fail.get() {
            return Ok(Some(&self.alternate));
        }
        self.inner.query(name)
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.inner.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.inner.scan(name, visitor)?;
        if self.fail.get() {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

#[test]
fn changing_definitions_and_late_source_failures_never_publish_a_cached_partial() {
    for alternate in [
        DYNAMIC.replace("PIVOT Quarter", "PIVOT Quarter+1"),
        "SELECT Region FROM Sales".into(),
    ] {
        let source = Changing {
            inner: Views::new(&[("Piv", DYNAMIC)]),
            alternate: parse_query(&alternate, Default::default())
                .unwrap()
                .into_query(),
            fail: Cell::new(false),
        };
        let query = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            execute_with_schema_discovery(query.query(), &source, &[], Default::default()).err(),
            Some(QueryError::SchemaChanged)
        );
    }
    let source = Changing {
        inner: Views::new(&[("Piv", DYNAMIC)]),
        alternate: parse_query(DYNAMIC, Default::default())
            .unwrap()
            .into_query(),
        fail: Cell::new(true),
    };
    let query = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
    assert_eq!(
        execute_with_schema_discovery(query.query(), &source, &[], Default::default()).err(),
        Some(QueryError::Source)
    );
    source.fail.set(false);
    assert!(execute_with_schema_discovery(query.query(), &source, &[], Default::default()).is_ok());
}
