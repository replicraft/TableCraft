use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn code_and_numeric_format_states_preserve_derived_errors_and_explicit_conversions() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Chr(CLng(CVErr(65))),Chr$(CLng(CVErr(66))),ChrW(CLng(CVErr(67))),FormatNumber(CLng(CVErr(11)),0)",
        Default::default(),
    ).unwrap();
    let options = ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        ..Default::default()
    };
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        &[vec![
            Value::Text("A".into()),
            Value::Text("B".into()),
            Value::Text("C".into()),
            Value::Text("11".into())
        ]]
    );
    for (sql, expected) in [
        (
            "SELECT Chr(E) FROM (SELECT CVErr(65) AS E) AS D",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT FormatNumber(E,256) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::Domain,
        ),
        (
            "SELECT FormatNumber(NULL,CVErr(11))",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT FormatNumber(CVErr(11),1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT FormatNumber(1,256,CVErr(11))", EvalError::Domain),
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
}
