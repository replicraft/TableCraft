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
fn date_and_text_options_keep_casing_and_raw_encoding_representations() {
    assert_eq!(run("SELECT StrConv('ABC',CDate(1.5),CDate(1033)),StrConv('ABC','1,5','1033'),AscB(StrConv('é','129','1033')),StrConv(StrConv('é€','128','1033'),'64','1033')", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Text("abc".into()),Value::Text("abc".into()),Value::Int16(201),Value::Text("é€".into())]]);
    assert_eq!(
        run("SELECT StrConv('ABC',CDate(1.5),CDate(1033))", None).unwrap(),
        [vec![Value::Text("abc".into())]]
    );
    assert_eq!(
        run(
            "SELECT Eval(\"StrConv('ABC','1,5','1033')\"),StrConv(NULL,'1,5','1033')",
            Some(NumberFormat::DeDe)
        )
        .unwrap(),
        [vec![Value::Text("abc".into()), Value::Null]]
    );
}

#[test]
fn prepared_options_rebind_locale_and_number_policy_without_changing_source_text() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS Flag TEXT,Locale TEXT; SELECT StrConv('I',Flag,Locale)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (locale, expected) in [("1033", "i"), ("1055", "ı")] {
        let bindings = [
            ("Flag", Value::Text("1.5".into())),
            ("Locale", Value::Text(locale.into())),
        ];
        let result = plan
            .execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Text(expected.into())]]);
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn eager_operands_and_option_failures_do_not_disappear_under_null_sources() {
    for sql in [
        "SELECT StrConv(NULL,'bad',1/0)",
        "SELECT StrConv('x','4',1/0)",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    for sql in [
        "SELECT StrConv('abc','1','1033')",
        "SELECT StrConv('abc',1,'1033')",
    ] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    for sql in [
        "SELECT StrConv(NULL,'32767.5','1033')",
        "SELECT StrConv(NULL,'4','1033')",
        "SELECT StrConv(NULL,'1','1041')",
        "SELECT StrConv('🦀','128','1033')",
    ] {
        assert!(run(sql, Some(NumberFormat::EnUs)).is_err(), "{sql}");
    }
}
