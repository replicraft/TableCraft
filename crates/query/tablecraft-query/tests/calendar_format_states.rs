use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::dates::parsing::DateTextFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn calendar_format_queries_retain_derived_states_and_eager_failures() {
    let source = MemorySource::default();
    let options = ExecutionOptions {
        date_format: Some(DateTextFormat::Iso),
        ..Default::default()
    };
    for (sql, expected) in [
        (
            "SELECT FormatDateTime(E,5) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::Domain,
        ),
        (
            "SELECT FormatDateTime(NULL,CVErr(11))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT FormatDateTime(CVErr(11),1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT MonthName(0,CVErr(11))", EvalError::TypeMismatch),
        ("SELECT WeekdayName(0,False,CVErr(11))", EvalError::Domain),
        ("SELECT MonthName(CVErr(11),1/0)", EvalError::DivisionByZero),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options);
        assert!(
            matches!(result, Err(QueryError::Evaluation(error)) if error == expected),
            "{sql}"
        );
    }
    let parsed = parse_query(
        "SELECT MonthName(CLng(CVErr(1))),WeekdayName(CLng(CVErr(1)),False,2),FormatDateTime(CDate(0.5),CLng(CVErr(4)))",
        Default::default(),
    ).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        &[vec![
            Value::Text("January".into()),
            Value::Text("Monday".into()),
            Value::Text("12:00".into())
        ]]
    );
}
