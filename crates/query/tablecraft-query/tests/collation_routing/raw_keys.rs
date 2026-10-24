use super::{
    raw_support::{RawResolver, Units},
    support::*,
};
use std::cell::Cell;
use tablecraft_expressions::EvalError;
use tablecraft_query::source::MemorySource;

const INPUT: &str = "(SELECT 1 AS ID,ChrW(55296)&'a' AS S UNION ALL SELECT 2,ChrW(55296)&'A' UNION ALL SELECT 3,ChrW(55297)&'a' UNION ALL SELECT 4,NULL) AS Q";

fn rows(sql: &str, options: ExecutionOptions<'_>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)?.execute(&source, &[], options)?;
    Ok(result.rows().to_vec())
}

fn literal_ids(values: &[i64]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|value| vec![Value::Int64(*value)])
        .collect()
}

#[test]
fn raw_keys_share_group_set_sort_distinctrow_and_top_semantics() {
    let options = ExecutionOptions {
        collation: Some(&Units(true)),
        ..Default::default()
    };
    for (sql, expected) in [
        (
            format!("SELECT Count(*),Min(ID),LenB(S) FROM {INPUT} GROUP BY S ORDER BY Min(ID)"),
            vec![
                vec![Value::Int64(2), Value::Int64(1), Value::Int32(4)],
                vec![Value::Int64(1), Value::Int64(3), Value::Int32(4)],
                vec![Value::Int64(1), Value::Int64(4), Value::Null],
            ],
        ),
        (
            format!("SELECT ID FROM {INPUT} ORDER BY S"),
            literal_ids(&[4, 1, 2, 3]),
        ),
        (
            format!("SELECT ID FROM {INPUT} ORDER BY S DESC"),
            literal_ids(&[3, 1, 2, 4]),
        ),
        (
            format!("SELECT TOP 1 ID FROM {INPUT} WHERE S IS NOT NULL ORDER BY S"),
            literal_ids(&[1, 2]),
        ),
        (
            format!("SELECT LenB(S) FROM (SELECT DISTINCT S FROM {INPUT}) AS D"),
            vec![vec![Value::Int32(4)], vec![Value::Int32(4)], vec![Value::Null]],
        ),
        (
            "SELECT Count(*) FROM (SELECT ChrW(55296) AS S UNION SELECT ChrW(55296)) AS Q".into(),
            vec![vec![Value::Int64(1)]],
        ),
        (
            "SELECT AscW(S) FROM (SELECT ChrW(55297) AS S UNION SELECT ChrW(55296)) AS Q ORDER BY S".into(),
            vec![vec![Value::Int16(-10240)], vec![Value::Int16(-10239)]],
        ),
        (
            "SELECT DISTINCTROW LenB(Q.S) FROM (SELECT ChrW(55296) AS S UNION ALL SELECT ChrW(55296) UNION ALL SELECT ChrW(55297)) AS Q INNER JOIN (SELECT 1 AS N UNION ALL SELECT 2) AS J ON True ORDER BY 1".into(),
            vec![vec![Value::Int32(2)], vec![Value::Int32(2)]],
        ),
    ] {
        assert_eq!(rows(&sql, options).unwrap(), expected, "{sql}");
    }
}

#[test]
fn raw_key_field_policies_and_unknown_conflicting_identities_stay_explicit() {
    let source = Fixture::new();
    let options = ExecutionOptions {
        collation_resolver: Some(&RawResolver),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT Min(Id),Count(*) FROM T GROUP BY A&ChrW(55296) ORDER BY Min(Id)",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], options)
        .unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![Value::Int32(1), Value::Int64(2)],
            vec![Value::Int32(3), Value::Int64(2)]
        ]
    );

    let parsed = parse_query(
        "SELECT Count(*) FROM (SELECT DISTINCT U&ChrW(55296) AS S FROM T WHERE Id=1) AS Q",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], options)
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int64(1)]]);

    for (sql, expected) in [
        (
            "SELECT Count(*) FROM T GROUP BY A&ChrW(55296)&B",
            QueryError::CollationConflict,
        ),
        (
            "SELECT Count(*) FROM (SELECT DISTINCT U&ChrW(55296) AS S FROM T) AS Q",
            QueryError::Evaluation(EvalError::CollationRequired),
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options);
        assert_eq!(result.err(), Some(expected), "{sql}");
    }
}

#[test]
fn fixed_named_crosstabs_retain_raw_row_and_cell_strings_for_outer_observers() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "TRANSFORM First(A&ChrW(55296)) SELECT Id FROM T GROUP BY Id PIVOT 'x' IN('x')",
        Default::default(),
    )
    .unwrap()
    .query()
    .clone();
    let parsed = parse_query(
        "SELECT Id,LenB([x]) FROM Named ORDER BY Id",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);

    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&RawResolver),
                ..Default::default()
            },
        )
        .unwrap();
    let expected: Vec<_> = (1..=4)
        .map(|id| vec![Value::Int32(id), Value::Int32(4)])
        .collect();
    assert_eq!(result.rows(), expected);
}

#[test]
fn raw_keys_preserve_scalar_provider_error_state_and_publication_boundaries() {
    let options = ExecutionOptions {
        collation: Some(&Units(false)),
        declared_integral: true,
        ..Default::default()
    };
    for sql in [
        "SELECT Count(*) FROM (SELECT DISTINCT CVErr(1) AS S) AS Q",
        "SELECT Count(*) FROM (SELECT DISTINCT ChrW(55296) AS S) AS Q",
        "SELECT Count(*) FROM (SELECT DISTINCT ChrB(65) AS S) AS Q",
    ] {
        assert_eq!(rows(sql, options).unwrap(), vec![vec![Value::Int64(1)]]);
    }
    for (sql, expected) in [
        (
            "SELECT DISTINCT ChrW(55296)",
            EvalError::UnrepresentableText,
        ),
        (
            "SELECT Count(*) FROM (SELECT ChrB(65) AS S UNION SELECT ChrB(65)) AS Q",
            EvalError::UnrepresentableText,
        ),
        (
            "SELECT Count(*) FROM (SELECT CVErr(1) AS S UNION SELECT NULL) AS Q",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Count(*) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q GROUP BY IIf(ID=1,CVar(ChrW(55296)),CVar(0))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT TOP 1 ID FROM (SELECT 1 AS ID,ChrW(55296) AS S UNION ALL SELECT 2,ChrW(1/0)) AS Q ORDER BY S",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(
            rows(sql, options).err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
    assert_eq!(
        rows(
            "SELECT Count(*) FROM (SELECT DISTINCT ChrW(55296) AS S) AS Q",
            Default::default()
        )
        .unwrap(),
        vec![vec![Value::Int64(1)]]
    );
}

#[test]
fn raw_key_work_can_cancel_at_every_observed_checkpoint() {
    let source = MemorySource::default();
    let sql = format!("SELECT Count(*),Min(ID) FROM {INPUT} GROUP BY S ORDER BY S");
    let parsed = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&Units(true)),
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Units(true)),
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
