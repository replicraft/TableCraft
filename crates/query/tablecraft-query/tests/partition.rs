use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn partition_states_keep_derived_error_null_gates_and_eager_failures() {
    assert_eq!(
        run("SELECT Partition(NULL,CVErr(11),9,2)").unwrap(),
        [vec![Value::Null]]
    );
    for (sql, expected) in [
        (
            "SELECT Partition(1,-1,9,CVErr(11))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Partition(E,0,9,2) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Partition(NULL,CVErr(11),9,1/0)",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(expected)));
    }
}

struct LabelOrder;

impl Collation for LabelOrder {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&LabelOrder),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn padded_labels_group_filter_correlate_and_retain_nulls() {
    assert_eq!(
        run("SELECT Partition(A.N,0,9,5),Count(*) FROM (SELECT 1 AS N UNION ALL SELECT 4 AS N UNION ALL SELECT 9 AS N) AS A GROUP BY Partition(A.N,0,9,5) ORDER BY 1").unwrap(),
        [
            vec![Value::Text(" 0: 4".into()), Value::Int64(2)],
            vec![Value::Text(" 5: 9".into()), Value::Int64(1)],
        ]
    );
    assert_eq!(
        run("SELECT (SELECT Partition(A.N,0,9,5)) FROM (SELECT 1 AS N UNION ALL SELECT 9 AS N) AS A WHERE Partition(A.N,0,9,5)=' 5: 9'").unwrap(),
        [vec![Value::Text(" 5: 9".into())]]
    );
    assert_eq!(
        run("SELECT Partition(Sum(A.N),0,9,5),Partition(Null,-1,0,0) FROM (SELECT 2 AS N UNION ALL SELECT 4 AS N) AS A").unwrap(),
        [vec![Value::Text(" 5: 9".into()), Value::Null]]
    );
    assert_eq!(
        run("SELECT Partition(A.N,0,9,5) FROM (SELECT 1 AS N) AS A GROUP BY Partition(A.N,0,9,4)")
            .err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn reusable_parameter_plans_keep_arity_and_eager_error_boundaries() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT Partition(N,0,9,5)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (number, expected) in [(1, " 0: 4"), (9, " 5: 9"), (10, "10:  ")] {
        let result = plan
            .execute(&source, &[("N", Value::Int32(number))], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Text(expected.into())]]);
    }

    for sql in [
        "SELECT Partition(Null,1/0,9,5)",
        "SELECT IIf(False,Partition(1/0,0,9,5),'unused')",
        "SELECT TOP 1 Partition(A.N,0,9,1/((A.N-2)*(A.N-2))) FROM (SELECT 1 AS N UNION ALL SELECT 2 AS N) AS A",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Partition(1,0,9)",
        "SELECT Partition(1,0,9,5,1)",
        "SELECT Partition(1,,9,5)",
        "SELECT Partition(*)",
        "SELECT VBA.Partition(1,0,9,5)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported), "{sql}");
    }
}

#[test]
fn range_output_shares_query_budgets_and_cancellation_checkpoints() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT Partition(50,0,99,5)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
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
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }

    let result = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                steps: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_)))
    ));
}

#[test]
fn partition_text_parameters_rebind_formats_without_changing_output_or_null_rules() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N TEXT; SELECT Partition(N,'0','9','2')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, input) in [(NumberFormat::EnUs, "2.5"), (NumberFormat::DeDe, "2,5")] {
        let result = plan
            .execute(
                &source,
                &[("N", Value::Text(input.into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Text(" 2: 3".into())]]);
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("N", Value::Text("2".into()))],
            Default::default()
        )
        .err(),
        Some(EvalError::NumberFormatRequired.into())
    );
    assert_eq!(
        plan.execute(&source, &[("N", Value::Null)], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Null]]
    );
}

#[test]
fn partition_text_retains_eager_errors_and_null_conversion_precedence() {
    for (sql, error) in [
        (
            "SELECT Partition('bad',NULL,9,1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT Partition('bad',0,9,1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()));
    }
    assert_eq!(
        run("SELECT Partition('bad',NULL,9,2)").unwrap(),
        &[vec![Value::Null]]
    );
}
