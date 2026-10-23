use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::text::casing::CaseLocale};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn unary_string_query_states_keep_derived_and_aggregate_errors_explicit() {
    let source = MemorySource::default();
    let locale: CaseLocale = "und".parse().unwrap();
    for function in [
        "Len",
        "LTrim",
        "RTrim",
        "Trim",
        "LCase",
        "UCase",
        "Asc",
        "AscW",
        "StrReverse",
        "Val",
    ] {
        for (operand, expected) in [
            ("CVErr(11)", EvalError::TypeMismatch),
            ("First(CVErr(11))", EvalError::TypeMismatch),
            ("IIf(True,CVErr(11),1/0)", EvalError::DivisionByZero),
        ] {
            let parsed =
                parse_query(&format!("SELECT {function}({operand})"), Default::default()).unwrap();
            let result = prepare(parsed.query(), &source).unwrap().execute(
                &source,
                &[],
                ExecutionOptions {
                    case_locale: Some(&locale),
                    ..Default::default()
                },
            );
            assert!(
                matches!(result, Err(QueryError::Evaluation(error)) if error == expected),
                "{function}({operand})"
            );
        }
        let parsed = parse_query(
            &format!("SELECT {function}(E) FROM (SELECT CVErr(11) AS E) AS D"),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(EvalError::TypeMismatch))
        ));
    }
    let parsed = parse_query(
        "SELECT Len(CStr(CVErr(11))),Trim(CStr(CVErr(11))),AscW(CStr(CVErr(11))),Val(CStr(CVErr(11)))",
        Default::default(),
    ).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![
            Value::Int32(8),
            Value::Text("Error 11".into()),
            Value::Int16(69),
            Value::Double(Double::from_float(0.0))
        ]]
    );
}
