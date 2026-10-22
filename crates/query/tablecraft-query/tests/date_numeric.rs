use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

#[test]
fn date_parameters_keep_exact_carriers_through_plan_reuse_and_closed_eval() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS D DATETIME; SELECT CByte(D),CInt(D),CLng(D),CLngLng(D),CBool(D),CSng(D),CDbl(D),CCur(D),CDec(D),IsNumeric(D),IsDate(D),CDate(CDbl(D)),Eval('CDbl(#1899-12-29 06:00:00#)')",
        Default::default(),
    ).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (serial, rounded) in [(2.5, 2), (3.5, 4)] {
        let value = date(serial);
        let result = plan
            .execute(&source, &[("D", value.clone())], Default::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Byte(rounded as u8),
                Value::Int16(rounded),
                Value::Int32(i32::from(rounded)),
                Value::Int64(i64::from(rounded)),
                Value::Boolean(true),
                Value::Single(Single::from_float(serial as f32)),
                Value::Double(Double::from_float(serial)),
                Value::Currency(Currency::from_scaled((serial * 10_000.0) as i64)),
                Value::Decimal(Decimal::new((serial * 10.0) as i128, 1).unwrap()),
                Value::Boolean(false),
                Value::Boolean(true),
                value,
                Value::Double(Double::from_float(-1.25)),
            ]]
        );
    }
    assert_eq!(
        plan.execute(&source, &[("D", date(45658.5))], Default::default())
            .err(),
        Some(EvalError::Overflow.into())
    );
    assert_eq!(
        plan.execute(&source, &[("D", Value::Null)], Default::default())
            .err(),
        Some(EvalError::InvalidNullConversion.into())
    );
}

#[test]
fn stored_dates_participate_in_numeric_filters_aggregates_groups_and_domains() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Times".into(),
            vec![Column::new("Stamp".into(), ScalarType::DateTime, true).unwrap()],
            vec![vec![date(2.5)], vec![date(-1.25)], vec![Value::Null]],
        )
        .unwrap(),
    ])
    .unwrap();
    for (sql, expected) in [
        (
            "SELECT CDbl(T.Stamp) FROM (SELECT Stamp FROM Times WHERE Stamp IS NOT NULL) AS T WHERE CDbl(T.Stamp)<0",
            vec![vec![Value::Double(Double::from_float(-1.25))]],
        ),
        (
            "SELECT Sum(CDbl(Stamp)) FROM Times WHERE Stamp IS NOT NULL",
            vec![vec![Value::Double(Double::from_float(1.25))]],
        ),
        (
            "SELECT CInt(Stamp),Count(*) FROM Times WHERE Stamp IS NOT NULL GROUP BY CInt(Stamp) ORDER BY CInt(Stamp)",
            vec![
                vec![Value::Int16(-1), Value::Int64(1)],
                vec![Value::Int16(2), Value::Int64(1)],
            ],
        ),
        (
            "SELECT DSum('CDbl(Stamp)','Times','Stamp Is Not Null')",
            vec![vec![Value::Double(Double::from_float(1.25))]],
        ),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let result = prepare(query.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions::default(),
        );
        assert_eq!(result.unwrap().rows(), expected, "{sql}");
    }
}

#[test]
fn an_eager_and_does_not_hide_invalid_null_conversion() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Times".into(),
            vec![Column::new("Stamp".into(), ScalarType::DateTime, true).unwrap()],
            vec![vec![Value::Null]],
        )
        .unwrap(),
    ])
    .unwrap();
    let query = parse_query(
        "SELECT Stamp FROM Times WHERE Stamp IS NOT NULL AND CDbl(Stamp)<0",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::InvalidNullConversion.into())
    );
}
