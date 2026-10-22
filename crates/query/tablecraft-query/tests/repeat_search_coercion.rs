use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn date_and_text_slots_keep_repetition_and_search_argument_identity() {
    assert_eq!(run("SELECT Space(CDate(2.5)),String(CDate(2.5),CDate(65.5)),InStr(CDate(2.5),'ababab','ab',0),InStrRev('ababab','ab',CDate(-1.25),0)", None).unwrap(), [vec![Value::Text("  ".into()),Value::Text("BB".into()),Value::Int32(3),Value::Int32(5)]]);
    assert_eq!(run("SELECT Space('2,5'),String('2,5','65'),InStr('2,5','ababab','ab',0),InStrRev('ababab','ab','4,5',0),Eval(\"String('2,5','x')\")", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Text("  ".into()),Value::Text("66".into()),Value::Int32(3),Value::Int32(3),Value::Text("xx".into())]]);
    assert_eq!(
        run("SELECT String('bad',NULL),Space(NULL)", None).unwrap(),
        [vec![Value::Null, Value::Null]]
    );
}

#[test]
fn text_start_plans_rebind_parameters_and_context_in_correlated_calls() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N TEXT; SELECT String(N,'x'),(SELECT InStr(N,'ababab','ab',0))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (text, repeated, position) in [("2.5", "xx", 3), ("4.5", "xxxx", 5)] {
        let bindings = [("N", Value::Text(text.into()))];
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(repeated.into()), Value::Int32(position)]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn eager_errors_null_starts_and_modes_do_not_turn_into_empty_results() {
    for sql in [
        "SELECT String('bad',1/0)",
        "SELECT InStr('bad','abc',1/0,0)",
        "SELECT InStrRev('abc',1/0,'bad',0)",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    for sql in [
        "SELECT Space('1')",
        "SELECT String('1','x')",
        "SELECT InStr('1','abc','b',0)",
        "SELECT InStrRev('abc','b','1',0)",
    ] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    for sql in [
        "SELECT InStr(NULL,NULL,'b',0)",
        "SELECT InStr('bad',NULL,'b',0)",
        "SELECT InStrRev('abc','b','bad',3)",
        "SELECT String('2.5','🦀')",
    ] {
        assert!(run(sql, Some(NumberFormat::EnUs)).is_err(), "{sql}");
    }
}
