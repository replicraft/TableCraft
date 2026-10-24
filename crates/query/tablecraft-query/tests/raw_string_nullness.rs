use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn raw_nullness_composes_in_filters_groups_correlations_and_aggregates() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT IsNull(ChrW(N)),ChrW(N) IS NOT NULL,(SELECT CStr(LeftB(ChrW(Q.N),1)) IS NULL) FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q WHERE ChrW(N) IS NOT NULL",
            vec![
                vec![
                    Value::Boolean(false),
                    Value::Boolean(true),
                    Value::Boolean(false)
                ];
                2
            ],
        ),
        (
            "SELECT IsNull(ChrW(N)),Count(*) FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q GROUP BY IsNull(ChrW(N)) HAVING NOT IsNull(ChrB(65))",
            vec![vec![Value::Boolean(false), Value::Int64(2)]],
        ),
        (
            "SELECT Count(IsNull(ChrW(N))),Count(ChrW(N)+NULL),IsNull(ChrW(55296)*NULL) FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q",
            vec![vec![Value::Int64(2), Value::Int64(0), Value::Boolean(true)]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();

        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn raw_nullness_rebinds_and_preserves_errors_and_cancellation() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT IsNull(ChrW(P)),LeftB(ChrW(P),1) IS NOT NULL",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for code in [65, 55296, 57343] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(code))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Boolean(false), Value::Boolean(true)]]
        );
    }
    for (input, error) in [
        (Value::Int32(65536), EvalError::Domain),
        (Value::Null, EvalError::InvalidNullConversion),
    ] {
        assert_eq!(
            plan.execute(&source, &[("P", input)], Default::default())
                .err(),
            Some(QueryError::Evaluation(error))
        );
    }

    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[("P", Value::Int32(55296))],
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
                &[("P", Value::Int32(55296))],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
