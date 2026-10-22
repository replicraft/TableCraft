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

fn source(input: &[(i32, i32)]) -> MemorySource {
    let columns = ["Kind", "Code"]
        .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
        .to_vec();
    let rows = input
        .iter()
        .map(|(kind, code)| vec![Value::Int32(*kind), Value::Int32(*code)])
        .collect();
    let table = MemoryTable::new("T".into(), columns, rows).unwrap();
    MemorySource::new(vec![table]).unwrap()
}

fn rows(source: &MemorySource, sql: &str) -> Result<Vec<Vec<ExpressionValue>>, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), source)?
        .execute_expression(source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn standard_error_extrema_preserve_codes_and_compose_with_explicit_consumers() {
    let source = source(&[(1, 65535), (2, 2001), (1, 0), (2, 2002)]);
    for sql in [
        "SELECT Min(CVErr(Code)),Max(CVErr(Code)) FROM T",
        "SELECT Min(Problem),Max(Problem) FROM (SELECT CVErr(Code) AS Problem FROM T) AS Q",
        "SELECT DMin('CVErr(Code)','T'),DMax('CVErr(Code)','T')",
    ] {
        assert_eq!(
            rows(&source, sql).unwrap(),
            [vec![
                ExpressionValue::Error(0),
                ExpressionValue::Error(65535)
            ]],
            "{sql}"
        );
    }
    assert_eq!(
        rows(
            &source,
            "SELECT CLng(Min(CVErr(Code))),CStr(Max(CVErr(Code))) FROM T"
        )
        .unwrap(),
        [vec![
            ExpressionValue::Value(Value::Int32(0)),
            ExpressionValue::Value(Value::Text("Error 65535".into())),
        ]]
    );
    assert_eq!(
        rows(&source, "SELECT Kind,Min(CVErr(Code)),Max(CVErr(Code)) FROM T GROUP BY Kind HAVING Min(CVErr(Code))>=CVErr(0) ORDER BY Kind").unwrap(),
        [
            vec![ExpressionValue::Value(Value::Int32(1)), ExpressionValue::Error(0), ExpressionValue::Error(65535)],
            vec![ExpressionValue::Value(Value::Int32(2)), ExpressionValue::Error(2001), ExpressionValue::Error(2002)],
        ]
    );
    assert_eq!(
        rows(
            &source,
            "TRANSFORM Min(CVErr(Code)) SELECT True AS G FROM T GROUP BY True PIVOT Kind IN (1,2,3)"
        )
        .unwrap(),
        [vec![
            ExpressionValue::Value(Value::Boolean(true)),
            ExpressionValue::Error(0),
            ExpressionValue::Error(2001),
            ExpressionValue::Value(Value::Null),
        ]]
    );
}

#[test]
fn nulls_empty_groups_and_scalar_publication_keep_their_separate_boundaries() {
    let source = source(&[(0, 0), (1, 2001), (0, 65535)]);
    assert_eq!(
        rows(
            &source,
            "SELECT Min(IIf(Kind=0,Null,CVErr(Code))),Max(IIf(Kind=0,Null,CVErr(Code))) FROM T"
        )
        .unwrap(),
        [vec![ExpressionValue::Error(2001); 2]]
    );
    for sql in [
        "SELECT Min(CVErr(-1)),Max(CVErr(-1)) FROM T WHERE Code<0",
        "SELECT Min(Null),Max(Null) FROM T",
    ] {
        assert_eq!(
            rows(&source, sql).unwrap(),
            [vec![ExpressionValue::Value(Value::Null); 2]]
        );
    }
    let query = parse_query("SELECT Min(CVErr(Code)) FROM T", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::NonScalarExpression.into())
    );
    for code in [0, 65535] {
        let singleton = self::source(&[(1, code)]);
        assert_eq!(
            rows(
                &singleton,
                "SELECT Min(CVErr(Code)),Max(CVErr(Code)) FROM T"
            )
            .unwrap(),
            [vec![ExpressionValue::Error(code as u32); 2]]
        );
    }
}

#[test]
fn mixed_kinds_and_late_operational_failures_are_never_discarded_by_extrema() {
    let source = source(&[(1, 2001), (2, 2002)]);
    for function in ["Min", "Max", "DMin", "DMax"] {
        let sql = if function.starts_with('D') {
            format!("SELECT {function}('IIf(Kind=1,CVErr(Code),Code)','T')")
        } else {
            format!("SELECT {function}(IIf(Kind=1,CVErr(Code),Code)) FROM T")
        };
        assert_eq!(
            rows(&source, &sql).err(),
            Some(EvalError::TypeMismatch.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Min(IIf(Kind=1,CVErr(Code),1/(2-Kind))) FROM T",
        "SELECT Max(IIf(Kind=1,CVErr(Code),1/(2-Kind))) FROM T",
        "SELECT DMin('IIf(Kind=1,CVErr(Code),1/(2-Kind))','T')",
        "SELECT DMax('IIf(Kind=1,CVErr(Code),1/(2-Kind))','T')",
    ] {
        assert_eq!(
            rows(&source, sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

#[test]
fn extrema_share_work_quotas_and_every_observed_cancellation_checkpoint() {
    let source = source(&[(1, 2001), (2, 2002)]);
    let query = parse_query(
        "SELECT Min(CVErr(Code)),DMax('CVErr(Code)','T') FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute_expression(
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
            plan.execute_expression(
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
            steps: 0,
            ..Default::default()
        },
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute_expression(
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

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4552_525f_4d49_4e4d),
        failure_persistence: None,
        ..Default::default()
    })]

    #[test]
    fn null_and_error_extrema_match_independent_code_selection(
        input in proptest::collection::vec((any::<bool>(), any::<u16>()), 0..32)
    ) {
        let data: Vec<_> = input.iter()
            .map(|(present, code)| (i32::from(*present), i32::from(*code)))
            .collect();
        let source = source(&data);
        let mut codes = input.iter().filter_map(|(present, code)| present.then_some(u32::from(*code)));
        let expected_min = codes.clone().min().map(ExpressionValue::Error)
            .unwrap_or(ExpressionValue::Value(Value::Null));
        let expected_max = codes.by_ref().max().map(ExpressionValue::Error)
            .unwrap_or(ExpressionValue::Value(Value::Null));
        let result = rows(&source, "SELECT Min(IIf(Kind=0,Null,CVErr(Code))),Max(IIf(Kind=0,Null,CVErr(Code))) FROM T").unwrap();
        prop_assert_eq!(result, [vec![expected_min, expected_max]]);
    }
}
