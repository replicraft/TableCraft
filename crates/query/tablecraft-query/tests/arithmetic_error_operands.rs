use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn error(sql: &str) -> QueryError {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), &source)
        .unwrap()
        .execute_expression(&source, &[], Default::default())
        .err()
        .unwrap()
}

#[test]
fn binary_and_unary_arithmetic_reject_error_codes_instead_of_publishing_them() {
    for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
        for (left, right) in [
            ("CVErr(0)", "1"),
            ("1", "CVErr(65535)"),
            ("CVErr(1)", "CVErr(2)"),
            ("Null", "CVErr(0)"),
            ("CVErr(0)", "Null"),
        ] {
            let sql = format!("SELECT {left} {operator} {right}");
            assert_eq!(error(&sql), EvalError::TypeMismatch.into(), "{sql}");
        }
    }
    for sql in ["SELECT +CVErr(0)", "SELECT -CVErr(65535)"] {
        assert_eq!(error(sql), EvalError::TypeMismatch.into(), "{sql}");
    }
}

#[test]
fn both_arithmetic_operands_evaluate_before_error_type_admission() {
    for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
        for (left, right, expected) in [
            ("CVErr(0)", "1/0", EvalError::DivisionByZero),
            ("1/0", "CVErr(0)", EvalError::DivisionByZero),
            ("CVErr(0)", "CVErr(-1)", EvalError::Domain),
            ("CVErr(-1)", "1/0", EvalError::Domain),
        ] {
            let sql = format!("SELECT ({left}) {operator} ({right})");
            assert_eq!(error(&sql), expected.into(), "{sql}");
        }
    }
}

#[test]
fn arithmetic_admission_follows_derived_aggregate_correlated_and_crosstab_operands() {
    for sql in [
        "SELECT Problem+1 FROM (SELECT CVErr(1) AS Problem) AS Q",
        "SELECT First(CVErr(ID))+1 FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
        "SELECT (SELECT CVErr(Q.ID))+1 FROM (SELECT 1 AS ID) AS Q",
        "SELECT Count(CVErr(ID)+1) FROM (SELECT 1 AS ID) AS Q",
        "TRANSFORM First(CVErr(ID))+1 SELECT True AS G FROM (SELECT 1 AS ID) AS Q GROUP BY True PIVOT ID IN (1)",
        "SELECT IsError(CVErr(1)+1)",
    ] {
        assert_eq!(error(sql), EvalError::TypeMismatch.into(), "{sql}");
    }
    let source = MemorySource::default();
    let query = parse_query("SELECT CLng(CVErr(1))+2", Default::default()).unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(3)]]);
}

#[test]
fn parameter_rebinding_and_every_observed_arithmetic_checkpoint_remain_explicit() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS Code LONG; SELECT CVErr(Code)+(CLng(CVErr(2))+1)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for code in [0, 65535, 1] {
        assert_eq!(
            plan.execute_expression(&source, &[("Code", Value::Int32(code))], Default::default())
                .err()
                .unwrap(),
            EvalError::TypeMismatch.into()
        );
    }

    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    assert_eq!(
        plan.execute_expression(
            &source,
            &[("Code", Value::Int32(1))],
            ExecutionOptions {
                cancelled: Some(&count),
                ..Default::default()
            },
        )
        .err()
        .unwrap(),
        EvalError::TypeMismatch.into()
    );
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            plan.execute_expression(
                &source,
                &[("Code", Value::Int32(1))],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                },
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
