use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source(rows: &[(i32, i32)]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Kind".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Code".into(), ScalarType::Int32, false).unwrap(),
            ],
            rows.iter()
                .map(|(kind, code)| vec![Value::Int32(*kind), Value::Int32(*code)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn rows(source: &MemorySource, sql: &str) -> Vec<Vec<ExpressionValue>> {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source)
        .unwrap()
        .execute_expression(source, &[], Default::default())
        .unwrap()
        .rows()
        .to_vec()
}

#[test]
fn encounter_and_domain_aggregates_preserve_codes_and_explicit_consumers() {
    let source = source(&[(1, 2001), (2, 2002), (1, 2003), (2, 2004)]);
    assert_eq!(
        rows(
            &source,
            "SELECT First(CVErr(Code)),Last(CVErr(Code)),IsError(First(CVErr(Code))),CLng(Last(CVErr(Code))) FROM Items"
        ),
        [vec![
            ExpressionValue::Error(2001),
            ExpressionValue::Error(2004),
            ExpressionValue::Value(Value::Boolean(true)),
            ExpressionValue::Value(Value::Int32(2004)),
        ]]
    );
    assert_eq!(
        rows(
            &source,
            "SELECT DFirst('CVErr(Code)','Items'),DLast('CVErr(Code)','Items'),DLookup('CVErr(Code)','Items','Code=2003'),IsError(DFirst('CVErr(Code)','Items'))"
        ),
        [vec![
            ExpressionValue::Error(2001),
            ExpressionValue::Error(2004),
            ExpressionValue::Error(2003),
            ExpressionValue::Value(Value::Boolean(true)),
        ]]
    );
    assert_eq!(
        rows(
            &source,
            "SELECT Kind,First(CVErr(Code)),Last(CVErr(Code)) FROM Items GROUP BY Kind HAVING IsError(First(CVErr(Code))) ORDER BY Kind"
        ),
        [
            vec![
                ExpressionValue::Value(Value::Int32(1)),
                ExpressionValue::Error(2001),
                ExpressionValue::Error(2003)
            ],
            vec![
                ExpressionValue::Value(Value::Int32(2)),
                ExpressionValue::Error(2002),
                ExpressionValue::Error(2004)
            ],
        ]
    );
    assert_eq!(
        rows(
            &source,
            "SELECT First(Q.Problem),Last(Q.Problem) FROM (SELECT CVErr(Code) AS Problem,Code FROM Items ORDER BY Code DESC) AS Q"
        ),
        [vec![
            ExpressionValue::Error(2004),
            ExpressionValue::Error(2001)
        ]]
    );
    assert_eq!(
        rows(
            &source,
            "TRANSFORM First(CVErr(Code)) SELECT Kind FROM Items GROUP BY Kind ORDER BY Kind PIVOT Kind IN (1,2)"
        ),
        [
            vec![
                ExpressionValue::Value(Value::Int32(1)),
                ExpressionValue::Error(2001),
                ExpressionValue::Value(Value::Null)
            ],
            vec![
                ExpressionValue::Value(Value::Int32(2)),
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(2002)
            ],
        ]
    );
}

#[test]
fn count_consumes_error_nullness_through_derived_domain_grouped_and_pivot_queries() {
    let source = source(&[(1, 2001), (2, 2002), (1, 2003), (2, 2004)]);
    for sql in [
        "SELECT Count(CVErr(Code)),Count(IIf(Kind=1,Null,CVErr(Code))),Count(*) FROM Items",
        "SELECT Count(Problem),Count(IIf(Kind=1,Null,Problem)),Count(*) FROM (SELECT CVErr(Code) AS Problem,Kind FROM Items) AS Q",
        "SELECT DCount('CVErr(Code)','Items'),DCount('CVErr(Code)','Items','Kind=2'),DCount('*','Items')",
    ] {
        assert_eq!(
            rows(&source, sql),
            [vec![
                ExpressionValue::Value(Value::Int64(4)),
                ExpressionValue::Value(Value::Int64(2)),
                ExpressionValue::Value(Value::Int64(4)),
            ]],
            "{sql}"
        );
    }
    assert_eq!(
        rows(
            &source,
            "SELECT Kind,Count(CVErr(Code)) FROM Items GROUP BY Kind HAVING Count(CVErr(Code))=2 ORDER BY Kind",
        ),
        [
            vec![
                ExpressionValue::Value(Value::Int32(1)),
                ExpressionValue::Value(Value::Int64(2))
            ],
            vec![
                ExpressionValue::Value(Value::Int32(2)),
                ExpressionValue::Value(Value::Int64(2))
            ],
        ]
    );
    assert_eq!(
        rows(
            &source,
            "TRANSFORM Count(CVErr(Code)) SELECT Kind,Count(CVErr(Code)) AS Total FROM Items GROUP BY Kind ORDER BY Kind PIVOT Kind IN (1,2,3)",
        ),
        [
            vec![
                ExpressionValue::Value(Value::Int32(1)),
                ExpressionValue::Value(Value::Int64(2)),
                ExpressionValue::Value(Value::Int64(2)),
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Value(Value::Null),
            ],
            vec![
                ExpressionValue::Value(Value::Int32(2)),
                ExpressionValue::Value(Value::Int64(2)),
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Value(Value::Int64(2)),
                ExpressionValue::Value(Value::Null),
            ],
        ]
    );
    assert_eq!(
        rows(
            &source,
            "SELECT Count(CVErr(-1)),Count(*) FROM Items WHERE Code=0"
        ),
        [vec![ExpressionValue::Value(Value::Int64(0)); 2]]
    );
}

#[test]
fn count_does_not_catch_operational_errors_or_convert_codes_for_numeric_aggregates() {
    let source = source(&[(1, 2001), (2, 2002)]);
    for sql in [
        "SELECT Count(IIf(Kind=1,CVErr(Code),1/(2-Kind))) FROM Items",
        "SELECT DCount('IIf(Kind=1,CVErr(Code),1/(2-Kind))','Items')",
        "TRANSFORM Count(IIf(Kind=1,CVErr(Code),1/(2-Kind))) SELECT True AS G FROM Items GROUP BY True PIVOT Kind IN (1,2)",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute_expression(&source, &[], Default::default())
                .err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for function in ["Sum", "Avg", "Min", "Max", "Var", "StDev"] {
        let query = parse_query(
            &format!("SELECT {function}(CVErr(Code)) FROM Items"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute_expression(&source, &[], Default::default())
                .err(),
            Some(EvalError::NonScalarExpression.into()),
            "{function}"
        );
    }
}

#[test]
fn nonselected_error_data_is_discardable_but_late_operational_failures_are_not() {
    let source = source(&[(1, 2001), (2, 2002)]);
    assert_eq!(
        rows(
            &source,
            "SELECT First(IIf(Kind=1,7,CVErr(Code))),Last(IIf(Kind=1,CVErr(Code),8)) FROM Items"
        ),
        [vec![
            ExpressionValue::Value(Value::Int64(7)),
            ExpressionValue::Value(Value::Int64(8))
        ]]
    );
    for sql in [
        "SELECT First(IIf(Kind=1,CVErr(Code),1/(2-Kind))) FROM Items",
        "SELECT Last(IIf(Kind=1,CVErr(Code),1/(2-Kind))) FROM Items",
        "SELECT DFirst('IIf(Kind=1,CVErr(Code),1/(2-Kind))','Items')",
        "SELECT DLast('IIf(Kind=1,CVErr(Code),1/(2-Kind))','Items')",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute_expression(&source, &[], Default::default())
                .err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        rows(
            &source,
            "SELECT First(CVErr(-1)),Last(CVErr(-1)) FROM Items WHERE Code=0"
        ),
        [vec![ExpressionValue::Value(Value::Null); 2]]
    );
    let query = parse_query("SELECT First(CVErr(Code)) FROM Items", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::NonScalarExpression.into())
    );
}

#[test]
fn aggregate_and_domain_checkpoints_share_cancellation_and_work_limits() {
    let source = source(&[(1, 2001), (2, 2002)]);
    let query = parse_query(
        "SELECT First(CVErr(Code)),DLast('CVErr(Code)','Items'),Count(CVErr(Code)),DCount('CVErr(Code)','Items') FROM Items",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    prepared
        .execute_expression(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&count),
                ..Default::default()
            },
        )
        .unwrap();
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            prepared.execute_expression(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 10,
            ..Default::default()
        },
    ] {
        assert!(
            prepared
                .execute_expression(
                    &source,
                    &[],
                    ExecutionOptions {
                        limits,
                        ..Default::default()
                    }
                )
                .is_err()
        );
    }
}

fn model(value: Option<&(i32, i32)>) -> ExpressionValue {
    match value {
        None | Some((0, _)) => ExpressionValue::Value(Value::Null),
        Some((1, code)) => ExpressionValue::Error(*code as u32),
        Some((_, code)) => ExpressionValue::Value(Value::Int32(*code)),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5641_5249_414e_5453),
        failure_persistence: None,
        ..Default::default()
    })]

    #[test]
    fn arbitrary_scalar_null_error_sequences_keep_their_encounter_endpoints(input in proptest::collection::vec((0_i32..3, 0_i32..=65535), 0..32)) {
        let source = source(&input);
        let expression = "IIf(Kind=0,Null,IIf(Kind=1,CVErr(Code),Code))";
        let actual = rows(&source, &format!("SELECT First({expression}),Last({expression}) FROM Items"));
        prop_assert_eq!(actual, [vec![model(input.first()), model(input.last())]]);

        let counted = rows(&source, &format!("SELECT Count({expression}),Count(*) FROM Items"));
        let non_null = input.iter().filter(|(kind, _)| *kind != 0).count() as i64;
        prop_assert_eq!(counted, [vec![
            ExpressionValue::Value(Value::Int64(non_null)),
            ExpressionValue::Value(Value::Int64(input.len() as i64)),
        ]]);
    }
}
