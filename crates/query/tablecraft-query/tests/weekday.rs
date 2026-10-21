use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::dates::{
        FirstDayOfWeek,
        parsing::{DateTextFormat, ReferenceYear},
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn weekday_dependencies_include_first_day_and_keep_group_identity() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Weekday(A.D,A.F),Count(*) FROM (SELECT #1969-02-12# AS D,2 AS F,999 AS Unused) AS A GROUP BY Weekday(A.D,A.F)",
            vec![Value::Int16(3), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT Weekday(A.D,A.F)) FROM (SELECT #1969-02-12# AS D,2 AS F,999 AS Unused) AS A",
            vec![Value::Int16(3)],
        ),
        (
            "SELECT Weekday(Max(A.D)) FROM (SELECT #1969-02-12# AS D) AS A",
            vec![Value::Int16(4)],
        ),
        (
            "SELECT Weekday(A.D,),Count(*) FROM (SELECT #1969-02-12# AS D) AS A GROUP BY Weekday(A.D)",
            vec![Value::Int16(4), Value::Int64(1)],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [expected]
        );
    }
    let query = parse_query(
        "SELECT Weekday(A.D,2) FROM (SELECT #1969-02-12# AS D) AS A GROUP BY Weekday(A.D,1)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}
#[test]
fn weekday_context_is_execution_local_and_does_not_capture_date_policy() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS D TEXT; SELECT Weekday(D,0)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (first, year, expected) in [(1, 2024, 5), (2, 2024, 4), (2, 2028, 2)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("D", Value::Text("Feb 29".into()))],
                ExecutionOptions {
                    first_day_of_week: Some(FirstDayOfWeek::new(first).unwrap()),
                    date_format: Some(DateTextFormat::EnUs),
                    reference_year: Some(ReferenceYear::new(year).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Int16(expected)]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("D", Value::Text("Feb 29".into()))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::CalendarPolicyRequired))
    );
}
