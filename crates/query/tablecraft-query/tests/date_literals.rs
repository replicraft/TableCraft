use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::{
        TwoDigitYearWindow,
        parsing::{DateTextFormat, ReferenceYear},
    },
};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, execute_with_schema_discovery, prepare,
    prepare_with_options, source::MemorySource,
};
use tablecraft_sql::parse_query;

#[test]
fn literal_values_are_independent_of_execution_date_policies_and_bind_without_rows() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Month(D.Stamp),Day(D.Stamp),Hour(D.Stamp) FROM (SELECT #1/2/2026 2:24 PM# AS Stamp) AS D",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for date_format in [None, Some(DateTextFormat::Iso), Some(DateTextFormat::DeDe)] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    date_format,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int16(1), Value::Int16(2), Value::Int16(14)]]
        );
    }
}

#[test]
fn invalid_date_literals_fail_metadata_binding_even_in_an_empty_result() {
    let source = MemorySource::default();
    for literal in [
        "2/29/1900",
        "1//2/26",
        "Mar 7",
        "32/13/2026",
        "12/31/2024 24:00",
    ] {
        let parsed = parse_query(
            &format!("SELECT #{literal}# WHERE False"),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source),
            Err(QueryError::Evaluation(EvalError::UnsupportedLiteral))
        ));
    }
}

#[test]
fn literal_dates_compose_with_grouped_subqueries_membership_and_dynamic_expressions() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Count(*) FROM (SELECT #1/2/2026# AS Stamp UNION ALL SELECT #Jan 2, 2026#) AS D WHERE Stamp BETWEEN #1/1/2026# AND #1/31/2026#",
            vec![Value::Int64(2)],
        ),
        (
            "SELECT (SELECT Day(#10/31/2026#)),#2/29/2000# IN (#2/28/2000#,#2000-02-29#),Eval('Month(#1/2/2026#)')",
            vec![Value::Int16(31), Value::Boolean(true), Value::Int16(1)],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
}

#[test]
fn token_years_are_not_rebound_by_runtime_date_serial_windows() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Year(#1/2/49#),Year(DateSerial(49,1,2)),Day(#31/12/26#)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (last, expected_constructor_year) in [(2029, 1949), (2049, 2049)] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    year_window: Some(TwoDigitYearWindow::new(last).unwrap()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Int16(1949),
                Value::Int16(expected_constructor_year),
                Value::Int16(26)
            ]]
        );
    }
}

#[test]
fn prepared_dates_capture_the_lexical_year_while_text_conversions_use_execution_context() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Year(#Mar 7#),Year(DateValue('Mar 7')),Year((SELECT #1/2#))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            reference_year: Some(ReferenceYear::new(2024).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();

    for runtime_year in [2023, 2030] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    reference_year: Some(ReferenceYear::new(runtime_year).unwrap()),
                    date_format: Some(DateTextFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Int16(2024),
                Value::Int16(runtime_year as i16),
                Value::Int16(2024)
            ]]
        );
    }
}

#[test]
fn staged_preparation_captures_an_explicit_year_and_rejects_invalid_tokens_before_scanning() {
    let source = MemorySource::default();
    let options = ExecutionOptions {
        reference_year: Some(ReferenceYear::new(2024).unwrap()),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT Day(#29/2#),Year(#Feb 29#),Year(#1/2/49#)",
        Default::default(),
    )
    .unwrap();
    let result = execute_with_schema_discovery(parsed.query(), &source, &[], options).unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            ExpressionValue::Value(Value::Int16(29)),
            ExpressionValue::Value(Value::Int16(2024)),
            ExpressionValue::Value(Value::Int16(1949))
        ]]
    );

    for sql in ["SELECT #2/29# WHERE False", "SELECT #1//2# WHERE False"] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert!(matches!(
            prepare_with_options(
                parsed.query(),
                &source,
                PreparationOptions {
                    reference_year: Some(ReferenceYear::new(2023).unwrap()),
                    ..Default::default()
                }
            ),
            Err(QueryError::Evaluation(EvalError::UnsupportedLiteral))
        ));
    }
}
