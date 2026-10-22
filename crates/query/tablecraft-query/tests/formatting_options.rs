use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, profile: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: profile,
                date_format: Some(DateTextFormat::Iso),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn contextual_formatting_slots_preserve_precision_defaults_and_date_profiles() {
    assert_eq!(run("SELECT FormatNumber(0.5,'2.5','0','-1','0'),FormatPercent(CDec(0.125),'2.5','-1','0','0'),FormatCurrency(CDec(-1.25),'1.5','-1','-1','0')", Some(NumberFormat::EnUs)).unwrap(), [vec![Value::Text(".50".into()),Value::Text("12.50%".into()),Value::Text("($1.25)".into())]]);
    assert_eq!(run("SELECT FormatDateTime(#2025-01-02 03:04:05#,'2,5'),FormatDateTime(#2025-01-02 03:04:05#,'3,5'),Eval(\"FormatNumber(1,'2,5')\")", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Text("2025-01-02".into()),Value::Text("03:04".into()),Value::Text("1,00".into())]]);
}

#[test]
fn prepared_format_options_rebind_values_and_number_syntax() {
    let source = MemorySource::default();
    let query = parse_query("PARAMETERS Digits TEXT; SELECT FormatNumber(1.25,Digits,'-1','0','0'),(SELECT FormatDateTime(#2025-01-02#,Digits))", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (digits, numeric, date) in [("2.5", "1.25", "2025-01-02"), ("3.5", "1.2500", "00:00")] {
        let bindings = [("Digits", Value::Text(digits.into()))];
        let options = ExecutionOptions {
            number_format: Some(NumberFormat::EnUs),
            date_format: Some(DateTextFormat::Iso),
            ..Default::default()
        };
        assert_eq!(
            plan.execute(&source, &bindings, options).unwrap().rows(),
            [vec![Value::Text(numeric.into()), Value::Text(date.into())]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn invalid_options_eager_operands_and_null_precedence_do_not_publish_results() {
    for sql in [
        "SELECT FormatNumber(NULL,'bad',1/0)",
        "SELECT FormatDateTime(NULL,1/0)",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        run(
            "SELECT FormatNumber(1,'256','bad')",
            Some(NumberFormat::EnUs)
        ),
        Err(QueryError::Evaluation(EvalError::Domain))
    );
    assert_eq!(
        run("SELECT FormatNumber(NULL,'bad')", None),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run(
            "SELECT FormatDateTime(NULL,'bad')",
            Some(NumberFormat::EnUs)
        ),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    for sql in [
        "SELECT FormatDateTime(#2025-01-02#,'2')",
        "SELECT FormatNumber(1,'2')",
    ] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    for sql in [
        "SELECT FormatNumber(1,'2','1')",
        "SELECT FormatDateTime(#2025-01-02#,'5')",
    ] {
        assert!(run(sql, Some(NumberFormat::EnUs)).is_err(), "{sql}");
    }
}
