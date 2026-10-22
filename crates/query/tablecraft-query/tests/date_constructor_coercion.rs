use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, dates::TwoDigitYearWindow},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn prepared_text_components_rebind_number_syntax_and_year_window() {
    let source = MemorySource::default();
    let query = parse_query(
        concat!(
            "PARAMETERS Y TEXT,M TEXT,D TEXT; SELECT DateSerial(Y,M,D),",
            "(SELECT DateSerial(Y,M,D)),Eval('TimeSerial(1.5,0,0)')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, month, day) in [
        (NumberFormat::EnUs, "2.5", "28.5"),
        (NumberFormat::DeDe, "2,5", "28,5"),
    ] {
        let bindings = [
            ("Y", Value::Text("49".into())),
            ("M", Value::Text(month.into())),
            ("D", Value::Text(day.into())),
        ];
        for (last, year) in [(2029, 1949), (2049, 2049)] {
            let options = ExecutionOptions {
                number_format: Some(profile),
                year_window: Some(TwoDigitYearWindow::new(last).unwrap()),
                ..Default::default()
            };
            let expected = Value::DateTime(
                LegacyDateTime::from_calendar(format!("{year}-02-28T00:00:00").parse().unwrap())
                    .unwrap(),
            );
            let time = Value::DateTime(LegacyDateTime::from_ole_days(2.0 / 24.0).unwrap());
            assert_eq!(
                plan.execute(&source, &bindings, options).unwrap().rows(),
                [vec![expected.clone(), expected, time]]
            );
        }
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
}

#[test]
fn contextual_constructor_inputs_survive_groups_filters_and_correlations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Year(DateSerial(Y,'2.5','28.5')),Count(*) FROM (SELECT '2024' AS Y,999 AS Unused) AS A WHERE Month(DateSerial(Y,'2.5','28.5'))=2 GROUP BY Year(DateSerial(Y,'2.5','28.5'))",
            vec![Value::Int16(2024), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT Hour(TimeSerial(A.H,'0','0'))) FROM (SELECT '1.5' AS H,999 AS Unused) AS A",
            vec![Value::Int16(2)],
        ),
        (
            "SELECT Day(DateSerial(CDate(2024.5),CDate(2.5),CDate(28.5)))",
            vec![Value::Int16(28)],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let options = ExecutionOptions {
            number_format: Some(NumberFormat::EnUs),
            ..Default::default()
        };
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options)
                .unwrap()
                .rows(),
            [expected]
        );
    }
}

#[test]
fn progressive_components_and_eager_expressions_keep_errors_visible() {
    let source = MemorySource::default();
    for (sql, error) in [
        ("SELECT DateSerial(32767.5,'bad',NULL)", EvalError::Overflow),
        (
            "SELECT DateSerial(NULL,'bad',NULL)",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT DateSerial('bad',NULL,NULL)",
            EvalError::NumberFormatRequired,
        ),
        ("SELECT DateSerial(NULL,1,1/0)", EvalError::DivisionByZero),
        ("SELECT TimeSerial(NULL,1/0,0)", EvalError::DivisionByZero),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(error)),
            "{sql}"
        );
    }
}
