use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn explicit_numeric_text_formats_reuse_parameter_plans_and_compose_through_sources() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT Hex(P),Oct(P),Str(P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, input) in [(NumberFormat::EnUs, "2.5"), (NumberFormat::DeDe, "2,5")] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(input.into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            &[vec![
                Value::Text("2".into()),
                Value::Text("2".into()),
                Value::Text(" 2.5".into())
            ]]
        );
    }
    let parsed = parse_query(
        "SELECT Hex(Q.N),Str(Q.N) FROM (SELECT '2,5' AS N) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    ));
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Text("2".into()), Value::Text(" 2.5".into())]]
    );
}

#[test]
fn str_preserves_error_and_null_expression_values_through_query_composition() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT Str(CVErr(2001)),Str(Nz(NULL,0)),Str(CVar(CVErr(0))),Str(NULL),Str(IIf(True,CVErr(65535),0))", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Text("Error 2001".into()),
            Value::Text(" 0".into()),
            Value::Text("Error 0".into()),
            Value::Null,
            Value::Text("Error 65535".into())
        ]]
    );
    for sql in [
        "SELECT Str(IIf(True,CVErr(1),1/0))",
        "SELECT Str(CVErr(1)),1/0",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(EvalError::DivisionByZero.into())
        );
    }
}

#[test]
fn str_dates_require_date_policy_without_requiring_numeric_format() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT Str(CDate(2))", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(EvalError::DateFormatRequired.into())
    );
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                date_format: Some(DateTextFormat::Iso),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Text("1900-01-01".into())]]);
}

#[test]
fn str_consumes_encounter_error_results_and_preserves_query_nz_text_identity() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Str(First(CVErr(Q.N))) FROM (SELECT 1 AS N UNION ALL SELECT 2 AS N) AS Q",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Text("Error 1".into())]]);
    let parsed = parse_query("SELECT Str(Nz(NULL))", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(EvalError::NumberFormatRequired.into())
    );
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(EvalError::TypeMismatch.into())
    );
}

#[test]
fn radix_dates_use_raw_long_conversion_and_error_requires_explicit_code_conversion() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT Hex(CDate(2.5)),Oct(CDate(8.5)),Hex(CDate(-1.5)),Hex(NULL),Oct(NULL),Hex(CLng(CVErr(1)))", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Text("2".into()),
            Value::Text("10".into()),
            Value::Text("FFFE".into()),
            Value::Null,
            Value::Null,
            Value::Text("1".into())
        ]]
    );
    for (sql, error) in [
        ("SELECT Hex(CVErr(1))", EvalError::TypeMismatch),
        ("SELECT Oct(CVErr(1))", EvalError::TypeMismatch),
        (
            "SELECT Hex(IIf(True,CVErr(1),1/0))",
            EvalError::DivisionByZero,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(error.into())
        );
    }
}
