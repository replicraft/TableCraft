use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::dates::{FirstDayOfWeek, parsing::DateTextFormat},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
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
