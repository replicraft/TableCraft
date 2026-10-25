use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn width_conversion_preserves_fields_raw_slices_aggregates_and_correlations() {
    assert_eq!(
        run("SELECT StrConv(A.T,4,1041),(SELECT StrConv(A.T,4,1041)) FROM (SELECT 'ABC' AS T UNION ALL SELECT NULL) AS A").unwrap(),
        [
            vec![Value::Text("ＡＢＣ".into()), Value::Text("ＡＢＣ".into())],
            vec![Value::Null, Value::Null],
        ]
    );
    assert_eq!(
        run("SELECT StrConv(First(A.T),8,1041),Count(*) FROM (SELECT 'カタカナ' AS T UNION ALL SELECT 'カタカナ') AS A GROUP BY Len(StrConv(A.T,8,1041))").unwrap(),
        [vec![Value::Text("ｶﾀｶﾅ".into()), Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT StrConv(CVar(LeftB('ｶﾞ',4)),4,1041),StrConv('ヷ',8,1041)").unwrap(),
        [vec![Value::Text("ガ".into()), Value::Text("ﾜﾞ".into())]]
    );
    for (sql, expected) in [
        (
            "SELECT StrConv(ChrW(55296),4,1041)",
            EvalError::UnrepresentableText,
        ),
        (
            "SELECT StrConv(ChrW(55296),4,1/0)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT StrConv(IIf(True,'あ',1/0),4,1041)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT StrConv(NULL,4,1033)",
            EvalError::UnsupportedExpression,
        ),
    ] {
        assert_eq!(run(sql), Err(expected.into()), "{sql}");
    }
}

#[test]
fn a_reused_width_plan_does_not_capture_mode_locale_or_failed_execution() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P TEXT,M LONG,L LONG; SELECT StrConv(P,M,L)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, mode, locale, expected) in [
        ("ABC", 4, 1041, Ok(Value::Text("ＡＢＣ".into()))),
        ("ＡＢＣ", 8, 1041, Ok(Value::Text("ABC".into()))),
        ("ABC", 4, 1033, Err(EvalError::UnsupportedExpression)),
        ("ABC", 5, 1041, Err(EvalError::UnsupportedExpression)),
        ("ABC", 2, 1033, Ok(Value::Text("abc".into()))),
        ("ＡＢＣ", 8, 1041, Ok(Value::Text("ABC".into()))),
    ] {
        let actual = plan.execute(
            &source,
            &[
                ("P", Value::Text(input.into())),
                ("M", Value::Int32(mode)),
                ("L", Value::Int32(locale)),
            ],
            ExecutionOptions::default(),
        );
        match expected {
            Ok(value) => assert_eq!(actual.unwrap().rows(), [vec![value]]),
            Err(error) => assert_eq!(actual.err(), Some(error.into())),
        }
    }
}
