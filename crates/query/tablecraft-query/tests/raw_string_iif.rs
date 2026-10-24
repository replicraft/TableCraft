use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn raw_iif_branches_retain_observations_and_publish_only_selected_values() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT LenB(IIf(True,ChrW(55296),ChrB(65))),LenB(IIf(False,ChrW(55296),ChrB(65))),VarType(IIf(True,ChrW(55296),1)),IsNull(IIf(False,ChrW(55296),NULL)),IIf(False,ChrW(55296),'ok')",
            vec![
                Value::Int32(2),
                Value::Int32(1),
                Value::Int16(8),
                Value::Boolean(true),
                Value::Text("ok".into()),
            ],
        ),
        (
            "SELECT IIf(True,ChrW(55357),ChrW(55296)) & ChrW(56832),AscB(IIf(False,ChrW(55296),ChrB(65)))",
            vec![Value::Text("😀".into()), Value::Int16(65)],
        ),
        (
            "SELECT Count(IIf(N=1,ChrW(55296),NULL)),Count(LeftB(IIf(True,ChrW(55296),ChrB(65)),1)) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            vec![Value::Int64(1), Value::Int64(2)],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();

        assert_eq!(result.rows(), &[expected], "{sql}");
    }
}

#[test]
fn raw_iif_keeps_eager_errors_and_strict_final_publication() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("LenB(IIf(True,ChrW(55296),1/0))", EvalError::DivisionByZero),
        ("LenB(IIf(False,ChrW(65536),ChrB(65)))", EvalError::Domain),
        ("IIf(True,ChrW(55296),'ok')", EvalError::UnrepresentableText),
        (
            "LenB(IIf(CVErr(1),ChrW(55296),ChrB(65)))",
            EvalError::TypeMismatch,
        ),
        (
            "LenB(IIf('True',ChrW(55296),ChrB(65)))",
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
fn raw_iif_rebinds_under_shared_quotas_and_cancellation() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P BIT; SELECT LenB(IIf(P,ChrW(55296),ChrB(65)))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (condition, length) in [(true, 2), (false, 1)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Boolean(condition))],
                Default::default()
            )
            .unwrap()
            .rows(),
            &[vec![Value::Int32(length)]]
        );
    }

    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[("P", Value::Boolean(true))],
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
                &[("P", Value::Boolean(true))],
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
            &[("P", Value::Boolean(true))],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .is_err()
    );
}
