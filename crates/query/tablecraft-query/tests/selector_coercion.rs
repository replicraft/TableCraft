use tablecraft_core::values::{ExpressionValue, LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn selector_policies_are_execution_local_and_preserve_error_data() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS Flag TEXT,Pick TEXT; SELECT IIf(Flag,CVErr(2001),CVErr(2003)),Switch(Flag,'yes',True,'no'),Choose(Pick,CVErr(1),CVErr(2002),CVErr(3)),Eval(\"IIf('1',CVErr(9),0)\")",
        Default::default(),
    ).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, flag, pick, error, word) in [
        (NumberFormat::EnUs, "0.25", "2.5", 2001, "yes"),
        (NumberFormat::DeDe, "0", "2,5", 2003, "no"),
    ] {
        let bindings = [
            ("Flag", Value::Text(flag.into())),
            ("Pick", Value::Text(pick.into())),
        ];
        let result = plan
            .execute_expression(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(profile),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                ExpressionValue::Error(error),
                ExpressionValue::Value(Value::Text(word.into())),
                ExpressionValue::Error(2002),
                ExpressionValue::Error(9),
            ]]
        );
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(profile),
                    ..Default::default()
                }
            )
            .err(),
            Some(EvalError::NonScalarExpression.into())
        );
    }
    assert_eq!(
        plan.execute_expression(
            &source,
            &[
                ("Flag", Value::Text("1".into())),
                ("Pick", Value::Text("1".into()))
            ],
            Default::default()
        )
        .err(),
        Some(EvalError::NumberFormatRequired.into())
    );
}

#[test]
fn date_conditions_work_in_groups_domains_derived_and_correlated_queries() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Times".into(),
            vec![Column::new("Stamp".into(), ScalarType::DateTime, true).unwrap()],
            [Some(0.0), Some(-0.25), None]
                .map(|serial| {
                    vec![serial.map_or(Value::Null, |serial| {
                        Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
                    })]
                })
                .to_vec(),
        )
        .unwrap(),
    ])
    .unwrap();
    for (sql, expected) in [
        (
            "SELECT IIf(Stamp,1,0),Count(*) FROM Times GROUP BY IIf(Stamp,1,0) ORDER BY IIf(Stamp,1,0)",
            vec![
                vec![Value::Int64(0), Value::Int64(2)],
                vec![Value::Int64(1), Value::Int64(1)],
            ],
        ),
        (
            "SELECT DSum('IIf(Stamp,1,0)','Times')",
            vec![vec![Value::Int64(1)]],
        ),
        (
            "SELECT Q.N FROM (SELECT Choose(CDate(2.5),1,2,3) AS N) AS Q",
            vec![vec![Value::Int64(2)]],
        ),
        (
            "SELECT (SELECT IIf(A.Stamp,1,0)) FROM Times AS A",
            vec![
                vec![Value::Int64(0)],
                vec![Value::Int64(1)],
                vec![Value::Int64(0)],
            ],
        ),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let result = prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn deferred_text_conversion_errors_keep_later_eager_random_draws() {
    let source = MemorySource::default();
    let query = parse_query("SELECT Switch('bad',Rnd(),True,Rnd())", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let state = RndState::default();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(EvalError::TypeMismatch.into())
    );
    let reference = RndState::default();
    reference.apply(None).unwrap();
    let expected = reference.apply(None).unwrap();
    assert_eq!(state.apply(Some(&Value::Int16(0))), Ok(expected));
}
