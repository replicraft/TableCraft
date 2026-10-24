use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn larger_retained_cells_keep_exact_lengths_across_chunked_copies() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT LenB(S),LenB(MidB(S,16001,402)) FROM (SELECT String(8200,'A')&ChrW(55296) AS S) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan.execute(&source, &[], Default::default()).unwrap();

    assert_eq!(
        result.rows(),
        &[vec![Value::Int32(16402), Value::Int32(402)]]
    );
}

#[test]
fn scalar_providers_keep_valid_string_key_results() {
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
            Ok(left.cmp(right))
        }
    }

    let source = MemorySource::default();
    for sql in [
        "SELECT LenB(S) FROM (SELECT DISTINCT ChrW(65) AS S) AS Q",
        "SELECT LenB(S) FROM (SELECT ChrW(65) AS S UNION SELECT ChrW(65)) AS Q",
        "SELECT LenB(S) FROM (SELECT ChrW(65) AS S) AS Q ORDER BY S",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int32(2)]], "{sql}");
    }
}

#[test]
fn retained_cell_copies_rebind_and_cancel_without_resetting_budgets() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT LenB(S),StrComp(S,ChrW(P),0) FROM (SELECT ChrW(P) AS S) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for code in [65, 55296, 57343] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(code))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(2), Value::Int16(0)]]
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

    assert!(
        plan.execute(
            &source,
            &[("P", Value::Int32(55296))],
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

#[test]
fn derived_string_cells_retain_raw_bytes_until_their_consumers() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(S) FROM (SELECT CStr(ChrW(55296)) AS S) AS Q",
            vec![vec![Value::Int32(1)]],
        ),
        (
            "SELECT Len(S) FROM (SELECT ChrW(55296)+'' AS S) AS Q",
            vec![vec![Value::Int32(1)]],
        ),
        (
            "SELECT Len(S) FROM (SELECT ChrW(55296)&'' AS S) AS Q",
            vec![vec![Value::Int32(1)]],
        ),
        (
            "SELECT LenB(S),VarType(S),IsNull(S) FROM (SELECT ChrW(55296) AS S) AS Q",
            vec![vec![
                Value::Int32(2),
                Value::Int16(8),
                Value::Boolean(false),
            ]],
        ),
        (
            "SELECT LenB(S),AscB(S) FROM (SELECT ChrB(65) AS S UNION ALL SELECT ChrB(66)) AS Q",
            vec![
                vec![Value::Int32(1), Value::Int16(65)],
                vec![Value::Int32(1), Value::Int16(66)],
            ],
        ),
        (
            "SELECT (SELECT LenB(Q.S)) FROM (SELECT ChrW(55296) AS S) AS Q",
            vec![vec![Value::Int32(2)]],
        ),
        (
            "SELECT LenB(First(S)),LenB(Last(S)) FROM (SELECT ChrB(65) AS S UNION ALL SELECT ChrW(55296)) AS Q",
            vec![vec![Value::Int32(1), Value::Int32(2)]],
        ),
        (
            "SELECT S,Trim(S) FROM (SELECT ChrW(65) AS S) AS Q",
            vec![vec![Value::Text("A".into()), Value::Text("A".into())]],
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
fn scalar_publication_stays_strict_after_retained_intermediate_rows() {
    let source = MemorySource::default();
    for sql in [
        "SELECT S FROM (SELECT ChrB(65) AS S) AS Q",
        "SELECT S FROM (SELECT ChrW(55296) AS S) AS Q",
        "SELECT Trim(S) FROM (SELECT ChrW(55296) AS S) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::UnrepresentableText)),
            "{sql}"
        );
        assert_eq!(
            plan.execute_expression(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::UnrepresentableText)),
            "{sql}"
        );
    }
}
