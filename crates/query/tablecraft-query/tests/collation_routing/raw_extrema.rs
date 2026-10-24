use super::{
    raw_support::{RawResolver, Units},
    support::*,
};
use std::cell::Cell;
use tablecraft_expressions::EvalError;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::source::MemorySource;

const INPUT: &str = "(SELECT 1 AS ID,ChrW(55296)&'b' AS S UNION ALL SELECT 2,ChrW(55296)&'a' UNION ALL SELECT 3,ChrW(55296)&'A' UNION ALL SELECT 4,NULL) AS Q";

fn rows(sql: &str, options: ExecutionOptions<'_>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)?.execute(&source, &[], options)?;
    Ok(result.rows().to_vec())
}

#[test]
fn raw_extrema_keep_representatives_through_observers_groups_and_derived_cells() {
    let options = ExecutionOptions {
        collation: Some(&Units(true)),
        ..Default::default()
    };
    for (sql, expected) in [
        (
            format!(
                "SELECT LenB(Min(S)),AscW(Min(S)),StrComp(Max(S),ChrW(55296)&'b',0),TypeName(Min(S)) FROM {INPUT}"
            ),
            vec![vec![
                Value::Int32(4),
                Value::Int16(-10240),
                Value::Int16(0),
                Value::Text("String".into()),
            ]],
        ),
        (
            format!(
                "SELECT LenB(L),LenB(H) FROM (SELECT Min(S) AS L,Max(S) AS H FROM {INPUT}) AS D"
            ),
            vec![vec![Value::Int32(4), Value::Int32(4)]],
        ),
        (
            format!(
                "SELECT Min(ID),LenB(Min(S)) FROM {INPUT} GROUP BY ID MOD 2 HAVING Min(S)=ChrW(55296)&'a' ORDER BY Min(ID)"
            ),
            vec![
                vec![Value::Int64(1), Value::Int32(4)],
                vec![Value::Int64(2), Value::Int32(4)],
            ],
        ),
    ] {
        assert_eq!(rows(&sql, options).unwrap(), expected, "{sql}");
    }

    let input = INPUT.strip_suffix(" AS Q").unwrap();
    let sql = format!(
        "SELECT O.ID,(SELECT LenB(Min(S)) FROM {input} AS I WHERE I.ID>=O.ID) FROM (SELECT 1 AS ID UNION ALL SELECT 4) AS O ORDER BY O.ID"
    );
    assert_eq!(
        rows(&sql, options).unwrap(),
        vec![
            vec![Value::Int64(1), Value::Int32(4)],
            vec![Value::Int64(4), Value::Null],
        ]
    );
}

#[test]
fn raw_extrema_null_and_singleton_data_need_no_unused_comparison() {
    for (sql, expected) in [
        (
            "SELECT LenB(Min(ChrB(65))),LenB(Max(ChrB(65))),LenB(Min(ChrW(55296)))",
            vec![vec![Value::Int32(1), Value::Int32(1), Value::Int32(2)]],
        ),
        (
            "SELECT LenB(Min(S)),LenB(Max(S)) FROM (SELECT ChrB(65) AS S UNION ALL SELECT NULL) AS Q",
            vec![vec![Value::Int32(1), Value::Int32(1)]],
        ),
        (
            "SELECT Min(S),Max(S) FROM (SELECT NULL AS S UNION ALL SELECT NULL) AS Q",
            vec![vec![Value::Null; 2]],
        ),
        (
            "SELECT Min(ChrW(65536)),Max(ChrW(65536)) FROM (SELECT 1 AS ID) AS Q WHERE False",
            vec![vec![Value::Null; 2]],
        ),
        (
            "SELECT Min(S),Max(S) FROM (SELECT 'b' AS S UNION ALL SELECT 'a') AS Q WHERE False",
            vec![vec![Value::Null; 2]],
        ),
    ] {
        assert_eq!(rows(sql, Default::default()).unwrap(), expected, "{sql}");
    }
}

#[test]
fn raw_domain_extrema_retain_field_policy_and_scalar_consumers_publish_valid_text() {
    let source = Fixture::new();
    let options = ExecutionOptions {
        collation: Some(&DEFAULT),
        collation_resolver: Some(&RawResolver),
        number_format: Some(NumberFormat::EnUs),
        ..Default::default()
    };
    for (sql, expected) in [
        (
            "SELECT DMin('A&ChrW(55296)','T')='A'&ChrW(55296),LenB(DMax('A&ChrW(55296)','T')),LenB(DMin('ChrB(65)','T','Id=1'))",
            vec![vec![Value::Boolean(true), Value::Int32(4), Value::Int32(1)]],
        ),
        (
            "SELECT DMin('ChrW(55296)','T','Id=0'),DMax('ChrW(55296)','T','Id=0')",
            vec![vec![Value::Null; 2]],
        ),
        (
            "SELECT Trim(DMin('ChrW(65)','T','Id=1')),Trim(Min(ChrW(65))) FROM T WHERE Id=1",
            vec![vec![Value::Text("A".into()), Value::Text("A".into())]],
        ),
        (
            "SELECT DCount(DMin('ChrW(65)','T','Id=1'),'T'),DCount(DMax('ChrW(65)','T','Id=1'),'T'),DCount(DLookup('ChrW(65)','T','Id=1'),'T')",
            vec![vec![Value::Int64(4); 3]],
        ),
        (
            "SELECT Count(*) FROM T HAVING Min(ChrW(49))",
            vec![vec![Value::Int64(4)]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan.execute(&source, &[], options).unwrap();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn raw_extrema_keep_strict_types_and_eager_operational_failures() {
    let options = ExecutionOptions {
        collation: Some(&Units(false)),
        declared_integral: true,
        ..Default::default()
    };
    for (sql, expected) in [
        ("SELECT Min(ChrW(55296))", EvalError::UnrepresentableText),
        (
            "SELECT LenB(Min(S)) FROM (SELECT ChrB(65) AS S UNION ALL SELECT ChrB(65)) AS Q",
            EvalError::UnrepresentableText,
        ),
        (
            "SELECT Min(IIf(ID=1,CVar(ChrW(55296)),CVar(0))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Min(IIf(ID=1,ChrW(55296),CVErr(1))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT Max(IIf(ID=1,CVErr(1),ChrW(55296))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT LenB(Min(ChrW(55296/(2-ID)))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
            EvalError::DivisionByZero,
        ),
        ("SELECT Min(ChrW(65536))", EvalError::Domain),
        ("SELECT Min(CVErr(65536))", EvalError::Domain),
    ] {
        assert_eq!(
            rows(sql, options).err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}

#[test]
fn raw_extrema_share_query_cancellation_at_every_observed_checkpoint() {
    let source = MemorySource::default();
    let parsed = parse_query(
        &format!("SELECT LenB(Min(S)),LenB(Max(S)) FROM {INPUT}"),
        Default::default(),
    )
    .unwrap();
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

#[test]
fn named_crosstab_raw_cells_feed_outer_and_domain_extrema_without_binding_scans() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "TRANSFORM First(A&ChrW(55296)) SELECT Id FROM T GROUP BY Id PIVOT 'x' IN('x')",
        Default::default(),
    )
    .unwrap()
    .query()
    .clone();
    let parsed = parse_query(
        "SELECT LenB(Min([x])),LenB(DMax('[x]','Named')) FROM Named",
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
    assert_eq!(result.rows(), &[vec![Value::Int32(4), Value::Int32(4)]]);
}

#[test]
fn crosstab_extrema_return_raw_cells_for_outer_observers() {
    for (aggregate, expected) in [("Min", "a"), ("Max", "b")] {
        let mut source = Fixture::new();
        let definition = format!(
            "TRANSFORM {aggregate}(A&ChrW(55296)) SELECT True AS G FROM T GROUP BY True PIVOT 'x' IN('x')"
        );
        source.named = parse_query(&definition, Default::default())
            .unwrap()
            .query()
            .clone();
        let sql = format!("SELECT LenB([x]),StrComp([x],'{expected}'&ChrW(55296),0) FROM Named");
        let parsed = parse_query(&sql, Default::default()).unwrap();
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
        assert_eq!(result.rows(), &[vec![Value::Int32(4), Value::Int16(0)]]);
    }
}
