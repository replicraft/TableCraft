use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, FirstWeekOfYear},
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::DeDe),
        first_day_of_week: Some(FirstDayOfWeek::new(2).unwrap()),
        first_week_of_year: Some(FirstWeekOfYear::new(2).unwrap()),
        ..Default::default()
    }
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(&source, &[], options())?
        .rows()
        .to_vec())
}

#[test]
fn week_text_and_date_options_flow_through_queries_and_closed_eval() {
    let sql = concat!(
        "SELECT DatePart('ww',#2021-01-01#,'2,5','1,5'),",
        "DateDiff('ww',#2026-10-18#,#2026-10-19#,'1,5','2,5'),",
        "Weekday(#2025-01-01#,CDate(2.5)),",
        "Format(#2021-01-01#,'w ww','2,5','1,5'),",
        "Eval(\"Format(#2021-01-01#,'w ww','2,5','1,5')\")"
    );
    assert_eq!(
        run(sql).unwrap(),
        [vec![
            Value::Int16(53),
            Value::Int32(1),
            Value::Int16(3),
            Value::Text("5 53".into()),
            Value::Text("5 53".into()),
        ]]
    );
    assert_eq!(
        run("SELECT DatePart('ww',#2021-01-01#,'-0,5','0,5')").unwrap(),
        [vec![Value::Int16(53)]]
    );
}

#[test]
fn prepared_options_rebind_and_reach_grouped_and_correlated_execution() {
    assert_eq!(
        run(
            "SELECT (SELECT Weekday(A.D,'1,5')) FROM (SELECT #2021-01-01# AS D,999 AS Unused) AS A"
        )
        .unwrap(),
        [vec![Value::Int16(5)]]
    );
    let source = MemorySource::default();
    let query = parse_query(
        concat!(
            "PARAMETERS F TEXT; SELECT DatePart('ww',D,F,'2'),",
            "Weekday(D,F),Count(*) ",
            "FROM (SELECT #2021-01-01# AS D,999 AS Unused) AS A ",
            "GROUP BY DatePart('ww',D,F,'2'),Weekday(D,F)"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (text, week, weekday) in [("1,5", 53, 5), ("3,5", 1, 3)] {
        let bindings = [("F", Value::Text(text.into()))];
        assert_eq!(
            plan.execute(&source, &bindings, options()).unwrap().rows(),
            [vec![
                Value::Int16(week),
                Value::Int16(weekday),
                Value::Int64(1)
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
fn validation_precedence_and_eager_failures_survive_contextual_options() {
    for (sql, error) in [
        (
            "SELECT DatePart('bad',NULL,'bad')",
            EvalError::InvalidDateInterval,
        ),
        ("SELECT DatePart('ww',NULL,'8','bad')", EvalError::Domain),
        (
            "SELECT DateDiff('ww',NULL,NULL,'8','bad')",
            EvalError::Domain,
        ),
        (
            "SELECT Format(NULL,'0','bad',NULL)",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT Weekday(NULL,'8')", EvalError::Domain),
        ("SELECT Weekday(NULL,NULL)", EvalError::TypeMismatch),
        (
            "SELECT DatePart('ww',NULL,'8',1/0)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT DateDiff('ww',NULL,NULL,'8',1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT Format(NULL,'0','8',1/0)", EvalError::DivisionByZero),
        ("SELECT Weekday(NULL,1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(error)), "{sql}");
    }
}
