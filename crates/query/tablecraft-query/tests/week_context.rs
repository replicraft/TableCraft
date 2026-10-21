use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::{FirstDayOfWeek, FirstWeekOfYear},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn prepared_plans_do_not_capture_system_week_settings() {
    let source = MemorySource::default();
    let query = parse_query("PARAMETERS D DATE; SELECT DatePart('ww',D,0,0),DatePart('w',D,0),DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0)", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let date = Value::DateTime(
        LegacyDateTime::from_calendar("2021-01-01T00:00:00".parse().unwrap()).unwrap(),
    );
    for (first, rule, week, weekday, difference) in
        [(1, 1, 1, 6, 0), (2, 2, 53, 5, 1), (2, 3, 52, 5, 1)]
    {
        assert_eq!(
            plan.execute(
                &source,
                &[("D", date.clone())],
                ExecutionOptions {
                    first_day_of_week: Some(FirstDayOfWeek::new(first).unwrap()),
                    first_week_of_year: Some(FirstWeekOfYear::new(rule).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![
                Value::Int16(week),
                Value::Int16(weekday),
                Value::Int32(difference)
            ]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("D", date)],
            ExecutionOptions {
                first_day_of_week: Some(FirstDayOfWeek::new(2).unwrap()),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::CalendarPolicyRequired))
    );
}
#[test]
fn week_context_flows_through_pruning_grouping_and_correlations() {
    let source = MemorySource::default();
    let settings = || ExecutionOptions {
        first_day_of_week: Some(FirstDayOfWeek::new(2).unwrap()),
        first_week_of_year: Some(FirstWeekOfYear::new(2).unwrap()),
        ..Default::default()
    };
    let query = parse_query("SELECT DatePart('ww',A.D,0,0),Count(*) FROM (SELECT #2021-01-01# AS D,999 AS Unused) AS A GROUP BY DatePart('ww',A.D,0,0)", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], settings())
            .unwrap()
            .rows(),
        [vec![Value::Int16(53), Value::Int64(1)]]
    );
    let query = parse_query("SELECT (SELECT DateDiff('ww',A.D,#2026-10-19#,0,0)) FROM (SELECT #2026-10-18# AS D,999 AS Unused) AS A", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], settings())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
}
