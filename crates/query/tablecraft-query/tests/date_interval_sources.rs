use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        date_format: Some(DateTextFormat::Iso),
        number_format: Some(NumberFormat::DeDe),
        ..Default::default()
    }
}

#[test]
fn prepared_sources_and_counts_rebind_without_capturing_format_policies() {
    let source = MemorySource::default();
    let query = parse_query(
        concat!(
            "PARAMETERS D TEXT,N TEXT; SELECT DateAdd('m',N,D),",
            "DatePart('d',D),DateDiff('h',D,DateAdd('d',1,D)),",
            "(SELECT DatePart('h',D)),Eval(\"DatePart('h','-1,25')\")"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, count, output, day, hour) in [
        ("2024-01-31 06:00:00", "1", "2024-02-29T06:00:00", 31, 6),
        ("2024-02-29 12:00:00", "0,5", "2024-02-29T12:00:00", 29, 12),
    ] {
        let bindings = [
            ("D", Value::Text(input.into())),
            ("N", Value::Text(count.into())),
        ];
        assert_eq!(
            plan.execute(&source, &bindings, options()).unwrap().rows(),
            [vec![
                Value::DateTime(LegacyDateTime::from_calendar(output.parse().unwrap()).unwrap()),
                Value::Int16(day),
                Value::Int32(24),
                Value::Int16(hour),
                Value::Int16(6),
            ]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
}

#[test]
fn converted_sources_work_in_filters_grouping_and_correlations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            concat!(
                "SELECT DatePart('m',D),Count(*) FROM (SELECT '2024-02-29' AS D,999 AS Unused) AS A ",
                "WHERE DateDiff('d',D,DateAdd('d','1',D))=1 GROUP BY DatePart('m',D)"
            ),
            vec![Value::Int16(2), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT DateDiff('h',A.D,DateAdd('d','1',A.D))) FROM (SELECT '0,25' AS D,999 AS Unused) AS A",
            vec![Value::Int32(24)],
        ),
        (
            "SELECT DatePart('h',-0.25),DatePart('d',DateAdd('d',CDate(2.5),0.25))",
            vec![Value::Int16(6), Value::Int16(1)],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .unwrap()
                .rows(),
            [expected]
        );
    }
}

#[test]
fn controls_both_diff_sources_and_eager_operands_keep_explicit_errors() {
    let source = MemorySource::default();
    for (sql, error) in [
        (
            "SELECT DateAdd('bad','bad','bad')",
            EvalError::InvalidDateInterval,
        ),
        ("SELECT DateAdd('d',NULL,'bad')", EvalError::TypeMismatch),
        ("SELECT DatePart('d','bad',8)", EvalError::Domain),
        (
            "SELECT DateDiff('w','bad','bad',2)",
            EvalError::UnsupportedExpression,
        ),
        ("SELECT DateDiff('d',NULL,'bad')", EvalError::TypeMismatch),
        ("SELECT DateDiff('d','bad',NULL)", EvalError::TypeMismatch),
        ("SELECT DateAdd('bad','bad',1/0)", EvalError::DivisionByZero),
        ("SELECT DatePart('bad',1/0)", EvalError::DivisionByZero),
        ("SELECT DateDiff('bad',NULL,1/0)", EvalError::DivisionByZero),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .err(),
            Some(QueryError::Evaluation(error)),
            "{sql}"
        );
    }
}
