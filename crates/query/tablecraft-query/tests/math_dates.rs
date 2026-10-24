use tablecraft_core::values::{Currency, Double, LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{OutputType, QueryError, prepare, source::MemorySource};
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn date_math_keeps_raw_serial_rules_and_types_through_derived_queries() {
    let result =
        run("SELECT Abs(Q.D),Int(Q.D),Fix(Q.D),Sgn(Q.D) FROM (SELECT CDate(-1.75) AS D) AS Q")
            .unwrap();
    assert_eq!(
        result,
        [vec![
            Value::Double(Double::from_float(1.75)),
            Value::DateTime(LegacyDateTime::from_ole_days(-2.0).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(-1.0).unwrap()),
            Value::Int16(-1),
        ]]
    );
    assert_eq!(
        run("SELECT Int(CDate(-657434.75))").err(),
        Some(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT Abs(CDate(1/0))").err(),
        Some(EvalError::DivisionByZero.into())
    );
}

#[test]
fn date_math_output_metadata_is_available_without_evaluating_overflowing_values() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("Abs(CDate(-657434.75))", ScalarType::Double),
        ("Int(CDate(-657434.75))", ScalarType::DateTime),
        ("Fix(CDate(-657434.75))", ScalarType::DateTime),
        ("Sgn(CDate(-657434.75))", ScalarType::Int16),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let types = plan.output_types().unwrap();
        let OutputType::Scalar { data_type, .. } = &types[0] else {
            panic!("scalar metadata expected")
        };
        assert_eq!(data_type.scalar_type(), expected);
    }
    assert_eq!(
        run("SELECT (SELECT Int(CDate(-657434.75)) WHERE False)").unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT Q.N FROM (SELECT Abs(CDate(-2.5)) AS N UNION ALL SELECT CInt(3)) AS Q")
            .unwrap(),
        [
            vec![Value::Double(Double::from_float(2.5))],
            vec![Value::Double(Double::from_float(3.0))]
        ]
    );
}

#[test]
fn round_date_digit_controls_compose_without_changing_source_type_or_eagerness() {
    let sql = "SELECT Round(CCur(2.25),Q.D),Round(NULL,Q.D),(SELECT Round(2.25,Q.D)) FROM (SELECT CDate(1) AS D) AS Q";
    assert_eq!(
        run(sql).unwrap(),
        [vec![
            Value::Currency(Currency::from_scaled(22_000)),
            Value::Null,
            Value::Double(Double::from_float(2.2)),
        ]]
    );

    for (sql, expected) in [
        ("SELECT Round(NULL,CDate(29))", EvalError::Domain),
        ("SELECT Round(CVErr(1),CDate(29))", EvalError::Domain),
        ("SELECT Round(CVErr(1),CDate(1))", EvalError::TypeMismatch),
        ("SELECT Round(1/0,CDate(29))", EvalError::DivisionByZero),
        (
            "SELECT TOP 1 Round(2.25,D) FROM (SELECT CDate(1) AS D UNION ALL SELECT CDate(29) AS D) AS Q",
            EvalError::Domain,
        ),
    ] {
        assert_eq!(
            run(sql).err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}
