use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::Collation,
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, parsing::DateTextFormat},
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

#[test]
fn parameters_follow_each_execution_language_and_host_week_setting() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N LONG; SELECT MonthName(N,True),WeekdayName(N)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, first, expected) in [
        (DateTextFormat::EnUs, 1, ["Jan", "Sunday"]),
        (DateTextFormat::DeDe, 2, ["Jan", "Montag"]),
        (DateTextFormat::Iso, 7, ["Jan", "Saturday"]),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("N", Value::Int32(1))],
                ExecutionOptions {
                    date_format: Some(profile),
                    first_day_of_week: Some(FirstDayOfWeek::new(first).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [expected.map(|name| Value::Text(name.into())).to_vec()]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("N", Value::Int32(1))],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::CalendarPolicyRequired))
    );
}
#[test]
fn names_retain_fields_optional_slots_grouping_identity_and_correlations() {
    let source = MemorySource::default();
    let settings = || ExecutionOptions {
        date_format: Some(DateTextFormat::EnUs),
        ..Default::default()
    };
    let query = parse_query("SELECT MonthName(A.N,),Count(*) FROM (SELECT 3 AS N,999 AS Unused) AS A GROUP BY MonthName(A.N)", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], settings())
            .unwrap()
            .rows(),
        [vec![Value::Text("March".into()), Value::Int64(1)]]
    );
    let query = parse_query(
        "SELECT (SELECT WeekdayName(A.N,,2)) FROM (SELECT 3 AS N,999 AS Unused) AS A",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], settings())
            .unwrap()
            .rows(),
        [vec![Value::Text("Wednesday".into())]]
    );
    let query = parse_query(
        "SELECT MonthName(A.N,True) FROM (SELECT 3 AS N) AS A GROUP BY MonthName(A.N,False)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn text_controls_follow_request_number_policy_and_recover_after_failure() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N TEXT; SELECT MonthName(N, 'True'), (SELECT WeekdayName('1', 'False', '2'))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, input, expected) in [
        (Some(NumberFormat::EnUs), "2.5", Ok("Feb")),
        (None, "2.5", Err(EvalError::NumberFormatRequired)),
        (
            Some(NumberFormat::DeDe),
            "2,5000000000000000000000000000000001",
            Ok("Mar"),
        ),
        (
            Some(NumberFormat::EnUs),
            "bad",
            Err(EvalError::TypeMismatch),
        ),
        (Some(NumberFormat::EnUs), "10", Ok("Oct")),
    ] {
        let result = plan.execute(
            &source,
            &[("N", Value::Text(input.into()))],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                number_format: format,
                ..Default::default()
            },
        );
        match expected {
            Ok(name) => assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Text(name.into()), Value::Text("Monday".into())]]
            ),
            Err(error) => assert_eq!(result.err(), Some(QueryError::Evaluation(error))),
        }
    }
}

#[test]
fn text_controls_compose_with_grouping_and_do_not_hide_late_rows() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT TOP 1 MonthName(N) FROM (SELECT '1' AS N UNION ALL SELECT 'bad' AS N) AS T",
            Err(EvalError::TypeMismatch),
        ),
        ("SELECT MonthName('1',1/0)", Err(EvalError::DivisionByZero)),
        (
            "SELECT IIf(False,MonthName('bad'),'ok')",
            Err(EvalError::TypeMismatch),
        ),
        (
            "SELECT MonthName(N),Count(*) FROM (SELECT '1' AS N UNION ALL SELECT '1' AS N) AS T GROUP BY MonthName(N)",
            Ok(vec![vec![Value::Text("January".into()), Value::Int64(2)]]),
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let result = prepare(query.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                number_format: Some(NumberFormat::EnUs),
                collation: Some(&Ordinal),
                ..Default::default()
            },
        );
        match expected {
            Ok(rows) => assert_eq!(result.unwrap().rows(), rows),
            Err(error) => assert_eq!(result.err(), Some(QueryError::Evaluation(error)), "{sql}"),
        }
    }
}
