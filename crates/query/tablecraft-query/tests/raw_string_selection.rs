use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn selectors_move_raw_choices_through_observations_and_aggregates() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT LenB(Nz(ChrW(55296),ChrB(65))),LenB(Nz(NULL,ChrB(65))),VarType(Choose(1,ChrW(55296),ChrB(65))),AscB(Switch(False,ChrW(55296),True,ChrB(65))),IsNull(Switch(True,NULL,True,ChrW(55296))),Nz('ok',ChrW(55296))",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Int32(2),
            Value::Int32(1),
            Value::Int16(8),
            Value::Int16(65),
            Value::Boolean(true),
            Value::Text("ok".into()),
        ]]
    );

    let parsed = parse_query(
        "SELECT Count(Nz(Choose(N,ChrW(55296),ChrB(65)),ChrW(57343))),Count(Switch(N=1,ChrW(55296))) FROM (SELECT 0 AS N UNION ALL SELECT 1 UNION ALL SELECT 2) AS Q",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int64(3), Value::Int64(1)]]
    );
}

#[test]
fn raw_selectors_keep_eager_errors_and_strict_publication() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("LenB(Nz(ChrW(55296),1/0))", EvalError::DivisionByZero),
        (
            "LenB(Choose(1e100,ChrW(55296),1/0))",
            EvalError::DivisionByZero,
        ),
        (
            "LenB(Switch(True,ChrW(55296),CVErr(1),1/0))",
            EvalError::DivisionByZero,
        ),
        ("Nz('ok',ChrW(65536))", EvalError::Domain),
        (
            "Choose(1,ChrW(55296),ChrB(65))",
            EvalError::UnrepresentableText,
        ),
        (
            "LenB(Switch(True,ChrW(55296),'True',ChrB(65)))",
            EvalError::NumberFormatRequired,
        ),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }
}

#[test]
fn raw_choose_rebinds_and_keeps_cumulative_limits_and_cancellation() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT IsNull(Choose(P,ChrW(55296),ChrB(65))),LenB(Nz(Choose(P,ChrW(55296),ChrB(65)),''))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (index, length) in [(0, 0), (1, 2), (2, 1), (3, 0)] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(index))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Boolean(length == 0), Value::Int32(length)]]
        );
    }

    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[("P", Value::Int32(1))],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[("P", Value::Int32(1))],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }

    assert!(
        plan.execute(
            &source,
            &[("P", Value::Int32(1))],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 0,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}
