use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

const MASK: &str = r#"yyyy;"negative";"epoch";"missing""#;

#[test]
fn numeric_serial_sections_reuse_prepared_parameters_and_natural_text_results() {
    let source = MemorySource::default();
    let sql = format!("PARAMETERS N DOUBLE; SELECT Format(N,'{MASK}'),VarType(Format(N,'{MASK}'))");
    let parsed = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for declared in [false, true] {
        for (value, expected) in [
            (Value::Double(Double::from_float(2.0)), "1900"),
            (Value::Double(Double::from_float(-2.0)), "negative"),
            (Value::Double(Double::from_float(0.0)), "epoch"),
            (Value::Null, "missing"),
        ] {
            let parameters = [("N", value)];
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            assert_eq!(
                plan.execute(&source, &parameters, options).unwrap().rows(),
                [vec![Value::Text(expected.into()), Value::Int16(8)]]
            );
        }
    }

    let sql = format!(
        "SELECT S FROM (SELECT Format(2,'{MASK}') AS S UNION ALL SELECT Format(-2,'{MASK}')) Q"
    );
    let parsed = parse_query(&sql, Default::default()).unwrap();
    let rows = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap()
        .rows()
        .to_vec();
    assert_eq!(
        rows,
        [
            vec![Value::Text("1900".into())],
            vec![Value::Text("negative".into())]
        ]
    );
}

#[test]
fn typed_date_and_negative_calendar_sections_stay_refused_with_eager_controls() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            format!("SELECT Format(#1900-01-01#,'{MASK}')"),
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT Format(-2,'yyyy;yyyy')".into(),
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT Format(0,'yyyy;yyyy')".into(),
            EvalError::UnsupportedExpression,
        ),
        (
            format!("SELECT Format(2,'{MASK}',1/0)"),
            EvalError::DivisionByZero,
        ),
        (format!("SELECT Format(2,'{MASK}',8)"), EvalError::Domain),
    ] {
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}

#[test]
fn numeric_date_section_work_and_publication_share_limits_and_cancellation() {
    let source = MemorySource::default();
    let sql = format!("SELECT Format(2,'{MASK}'),Format(-2,'{MASK}')");
    let parsed = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = observed.get();
    for stop in 1..=total {
        observed.set(0);
        let cancel = || {
            observed.set(observed.get() + 1);
            observed.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
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
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![
            Value::Text("1900".into()),
            Value::Text("negative".into())
        ]]
    );
}
